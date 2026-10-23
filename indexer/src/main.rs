#[cfg(test)]
mod chaos_test;
#[cfg(test)]
mod e2e_test;

use std::{env, time::Duration};

use anyhow::{Context, Result};
use axum::{extract::State, routing::get, Json, Router};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{postgres::PgPoolOptions, PgPool};
use tokio::{net::TcpListener, time::sleep};
use tracing::{error, info, warn};

// ---------------------------------------------------------------------------
// Config & environment variable validation
// ---------------------------------------------------------------------------

struct Config {
    database_url: String,
    horizon_url: String,
    allowed_horizon_domains: Vec<String>,
    contract_id: String,
    poll_interval: Duration,
    backend_url: Option<String>,
}

/// Required environment variables that must be set before the indexer starts.
const REQUIRED_ENV_VARS: &[(&str, &str)] = &[
    ("DATABASE_URL", "PostgreSQL connection string, e.g. postgres://user@localhost/votechain (supply the password via DATABASE_PASSWORD_FILE)"),
    ("CONTRACT_ID", "Deployed VoteChain governance contract address (C...)"),
];

/// Validates all required environment variables up-front and returns a
/// descriptive error listing every missing variable, rather than failing on
/// the first missing one with a cryptic message.
///
/// # Errors
/// Returns an error if any required variable is absent or empty, with a
/// human-readable list of what is missing and where to find reference values.
fn validate_env() -> Result<()> {
    let missing: Vec<(&str, &str)> = REQUIRED_ENV_VARS
        .iter()
        .filter(|(key, _)| env::var(key).map(|v| v.is_empty()).unwrap_or(true))
        .cloned()
        .collect();

    if !missing.is_empty() {
        let details = missing
            .iter()
            .map(|(key, desc)| format!("  • {key}\n      {desc}"))
            .collect::<Vec<_>>()
            .join("\n");
        anyhow::bail!(
            "Indexer startup failed — missing required environment variables:\n\n\
             {details}\n\n\
             Set these variables before starting the indexer.\n\
             See .env.example for reference values."
        );
    }
    Ok(())
}

/// Validates the format of a PostgreSQL connection string and, if
/// `password_file` is set, injects the password read from that file.
///
/// Error messages never include the URL itself, so credentials cannot leak
/// into logs.
fn resolve_database_url(raw: &str, password_file: Option<&str>) -> Result<String> {
    let mut url = url::Url::parse(raw)
        .map_err(|_| anyhow::anyhow!("DATABASE_URL is not a valid URL (value redacted)"))?;
    if !matches!(url.scheme(), "postgres" | "postgresql") {
        anyhow::bail!("DATABASE_URL must use the postgres:// or postgresql:// scheme");
    }
    if url.host_str().map(str::is_empty).unwrap_or(true) {
        anyhow::bail!("DATABASE_URL must include a host");
    }
    if let Some(path) = password_file.filter(|p| !p.is_empty()) {
        if url.password().is_some() {
            anyhow::bail!("DATABASE_URL must not embed a password when DATABASE_PASSWORD_FILE is set");
        }
        let password = std::fs::read_to_string(path)
            .context("failed to read DATABASE_PASSWORD_FILE")?;
        let password = password.trim_end_matches(['\n', '\r']);
        if password.is_empty() {
            anyhow::bail!("DATABASE_PASSWORD_FILE is empty");
        }
        url.set_password(Some(password))
            .map_err(|_| anyhow::anyhow!("DATABASE_URL cannot carry a password"))?;
    }
    Ok(url.into())
}

/// Default allowed Horizon domains used when ALLOWED_HORIZON_DOMAINS is unset.
const DEFAULT_ALLOWED_HORIZON_DOMAINS: &str =
    "horizon-testnet.stellar.org,horizon.stellar.org";

/// Validates a Horizon URL for SSRF safety.
///
/// Checks performed (in order):
/// 1. The URL must parse successfully.
/// 2. The scheme must be `https`.
/// 3. The hostname must appear in the `allowed_domains` allowlist.
/// 4. If the hostname is a literal IP address it is checked against blocked
///    private/loopback ranges (RFC 1918, RFC 4193, link-local, loopback).
///    DNS-resolved IPs are **not** checked at startup (see documentation
///    limitation note in `.env.example`).
///
/// # Errors
/// Returns a descriptive error for each validation failure so the operator
/// can fix the misconfiguration immediately.
fn validate_horizon_url(raw: &str, allowed_domains: &[String]) -> Result<()> {
    let parsed = url::Url::parse(raw)
        .map_err(|_| anyhow::anyhow!("HORIZON_URL is not a valid URL"))?;

    // 1. Require HTTPS.
    if parsed.scheme() != "https" {
        anyhow::bail!(
            "HORIZON_URL must use the https:// scheme (got '{}://'). \
             Plain HTTP is not permitted in production.",
            parsed.scheme()
        );
    }

    // 2. Extract the hostname.
    let host = parsed
        .host_str()
        .filter(|h| !h.is_empty())
        .ok_or_else(|| anyhow::anyhow!("HORIZON_URL must include a host"))?;

    // 3. Check domain allowlist (case-insensitive, strip optional trailing dot).
    let host_normalised = host.trim_end_matches('.').to_ascii_lowercase();
    let allowed = allowed_domains
        .iter()
        .any(|d| d.trim().to_ascii_lowercase() == host_normalised);
    if !allowed {
        anyhow::bail!(
            "HORIZON_URL host '{}' is not in the allowed domains list. \
             Permitted hosts: {}. \
             Override via ALLOWED_HORIZON_DOMAINS (comma-separated).",
            host,
            allowed_domains.join(", ")
        );
    }

    // 4. Block literal private / loopback IP addresses.
    //    We use the `url` crate's host parsing to detect IPv4/IPv6 literals.
    use url::Host;
    match parsed.host() {
        Some(Host::Ipv4(ip)) => {
            let octets = ip.octets();
            let is_private = matches!(octets,
                // 10.0.0.0/8
                [10, ..] |
                // 172.16.0.0/12  (172.16.x.x – 172.31.x.x)
                [172, 16..=31, ..] |
                // 192.168.0.0/16
                [192, 168, ..] |
                // 127.0.0.0/8 loopback
                [127, ..] |
                // 169.254.0.0/16 link-local
                [169, 254, ..]
            );
            if is_private {
                anyhow::bail!(
                    "HORIZON_URL resolves to a private/loopback IPv4 address ({}). \
                     Connection to private IP ranges is blocked to prevent SSRF.",
                    ip
                );
            }
        }
        Some(Host::Ipv6(ip)) => {
            let segments = ip.segments();
            // ::1 loopback
            let is_loopback = ip.is_loopback();
            // fc00::/7 unique local (fc00:: – fdff::)
            let is_unique_local = (segments[0] & 0xfe00) == 0xfc00;
            // fe80::/10 link-local
            let is_link_local = (segments[0] & 0xffc0) == 0xfe80;
            if is_loopback || is_unique_local || is_link_local {
                anyhow::bail!(
                    "HORIZON_URL resolves to a private/loopback IPv6 address ({}). \
                     Connection to private IP ranges is blocked to prevent SSRF.",
                    ip
                );
            }
        }
        _ => {} // Hostname — DNS resolution not validated at startup (see docs limitation)
    }

    Ok(())
}

impl Config {
    fn from_env() -> Result<Self> {
        // Validate all required vars first so the operator sees every missing
        // variable in a single error, not one at a time.
        validate_env()?;

        let database_url = resolve_database_url(
            &env::var("DATABASE_URL").context("DATABASE_URL must be set")?,
            env::var("DATABASE_PASSWORD_FILE").ok().as_deref(),
        )?;
        // Scrub credentials from the process environment so they are not
        // exposed via /proc/self/environ to anything that inspects it later.
        env::remove_var("DATABASE_URL");
        env::remove_var("DATABASE_PASSWORD_FILE");

        let allowed_horizon_domains: Vec<String> = env::var("ALLOWED_HORIZON_DOMAINS")
            .unwrap_or_else(|_| DEFAULT_ALLOWED_HORIZON_DOMAINS.into())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let horizon_url = env::var("HORIZON_URL")
            .unwrap_or_else(|_| "https://horizon-testnet.stellar.org".into());
        validate_horizon_url(&horizon_url, &allowed_horizon_domains)?;

        Ok(Self {
            database_url,
            horizon_url,
            allowed_horizon_domains,
            contract_id: env::var("CONTRACT_ID").context("CONTRACT_ID must be set")?,
            poll_interval: Duration::from_secs(
                env::var("POLL_INTERVAL_SECS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(3),
            ),
            backend_url: env::var("BACKEND_URL").ok(),
        })
    }
}

// ---------------------------------------------------------------------------
// Horizon response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct HorizonEventsPage {
    #[serde(rename = "_embedded")]
    embedded: Embedded,
}

#[derive(Deserialize)]
struct Embedded {
    records: Vec<HorizonEvent>,
}

#[derive(Deserialize)]
struct HorizonEvent {
    ledger: u64,
    transaction_hash: String,
    #[serde(rename = "contract_id")]
    contract_id: String,
    topic: Vec<Value>,
    value: Value,
}

// ---------------------------------------------------------------------------
// DB helpers
// ---------------------------------------------------------------------------

async fn last_ledger(pool: &PgPool, contract_id: &str) -> Result<u64> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT last_ledger FROM indexer_cursor WHERE contract_id = $1")
            .bind(contract_id)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|(l,)| l as u64).unwrap_or(0))
}

async fn save_cursor(pool: &PgPool, contract_id: &str, ledger: u64) -> Result<()> {
    sqlx::query(
        "INSERT INTO indexer_cursor (contract_id, last_ledger)
         VALUES ($1, $2)
         ON CONFLICT (contract_id) DO UPDATE SET last_ledger = EXCLUDED.last_ledger",
    )
    .bind(contract_id)
    .bind(ledger as i64)
    .execute(pool)
    .await?;
    Ok(())
}

async fn insert_event(pool: &PgPool, ev: &HorizonEvent, topic: &str) -> Result<()> {
    let proposal_id: Option<i64> = ev.topic.get(1).and_then(|v| v.as_i64());
    sqlx::query(
        "INSERT INTO contract_events
             (ledger_seq, tx_hash, contract_id, topic, proposal_id, payload)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT DO NOTHING",
    )
    .bind(ev.ledger as i64)
    .bind(&ev.transaction_hash)
    .bind(&ev.contract_id)
    .bind(topic)
    .bind(proposal_id)
    .bind(&ev.value)
    .execute(pool)
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Cache invalidation helper
// ---------------------------------------------------------------------------

/// Fire-and-forget POST to the backend cache invalidation endpoint.
/// Errors are logged as warnings and never propagate to the caller.
async fn call_invalidate(client: &Client, backend_url: &str, proposal_id: Option<i64>) {
    let url = format!("{}/api/proposals/invalidate", backend_url);
    let body = match proposal_id {
        Some(id) => serde_json::json!({ "id": id }),
        None => serde_json::json!({}),
    };
    match client
        .post(&url)
        .json(&body)
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            info!(proposal_id, "cache invalidated via backend");
        }
        Ok(resp) => {
            warn!(status = %resp.status(), "cache invalidation returned non-2xx");
        }
        Err(e) => {
            warn!("cache invalidation request failed: {e:#}");
        }
    }
}

// ---------------------------------------------------------------------------
// Ingestion loop
// ---------------------------------------------------------------------------

async fn ingest(pool: PgPool, cfg: Config) {
    let client = Client::new();
    loop {
        match poll_once(&client, &pool, &cfg).await {
            Ok(count) => {
                if count > 0 {
                    info!(count, "ingested events");
                }
            }
            Err(e) => error!("poll error: {e:#}"),
        }
        sleep(cfg.poll_interval).await;
    }
}

async fn poll_once(client: &Client, pool: &PgPool, cfg: &Config) -> Result<usize> {
    let cursor = last_ledger(pool, &cfg.contract_id).await?;

    let url = format!(
        "{}/contracts/{}/events?cursor={}&limit=200&order=asc",
        cfg.horizon_url, cfg.contract_id, cursor
    );

    let page: HorizonEventsPage = client
        .get(&url)
        .timeout(Duration::from_secs(10))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let records = page.embedded.records;
    if records.is_empty() {
        return Ok(0);
    }

    let mut max_ledger = cursor;
    let mut count = 0;

    for ev in &records {
        let topic = ev
            .topic
            .first()
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        match topic {
            "init" | "created" | "vote" | "final" | "executed" | "cancelled" | "qupdate"
            | "admxfer" | "paused" | "unpaused" | "durationupdate" => {
                insert_event(pool, ev, topic).await?;
                count += 1;

                // Invalidate the backend Redis cache for events that change proposal state.
                if matches!(topic, "created" | "vote" | "final" | "executed" | "cancelled") {
                    if let Some(ref backend_url) = cfg.backend_url {
                        let proposal_id: Option<i64> = ev.topic.get(1).and_then(|v| v.as_i64());
                        let client = client.clone();
                        let backend_url = backend_url.clone();
                        tokio::spawn(async move {
                            call_invalidate(&client, &backend_url, proposal_id).await;
                        });
                    }
                }
            }
            other => warn!(other, "unknown event topic — skipping"),
        }

        if ev.ledger > max_ledger {
            max_ledger = ev.ledger;
        }
    }

    save_cursor(pool, &cfg.contract_id, max_ledger).await?;
    Ok(count)
}

// ---------------------------------------------------------------------------
// REST API
// ---------------------------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
struct EventRow {
    id: i64,
    ledger_seq: i64,
    tx_hash: String,
    topic: String,
    proposal_id: Option<i64>,
    payload: Value,
    ingested_at: chrono::DateTime<chrono::Utc>,
}

async fn list_events(State(pool): State<PgPool>) -> Json<Vec<EventRow>> {
    let rows = sqlx::query_as::<_, EventRow>(
        "SELECT id, ledger_seq, tx_hash, topic, proposal_id, payload, ingested_at
         FROM contract_events ORDER BY ledger_seq DESC LIMIT 100",
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();
    Json(rows)
}

async fn list_proposal_events(
    State(pool): State<PgPool>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Json<Vec<EventRow>> {
    let rows = sqlx::query_as::<_, EventRow>(
        "SELECT id, ledger_seq, tx_hash, topic, proposal_id, payload, ingested_at
         FROM contract_events WHERE proposal_id = $1 ORDER BY ledger_seq ASC",
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();
    Json(rows)
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cfg = Config::from_env()?;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cfg.database_url)
        .await
        .context("connect to postgres")?;

    // Run database migrations using sqlx's built-in migration runner.
    //
    // sqlx::migrate!() embeds all *.sql files from the migrations/ directory
    // at compile time, tracks which have been applied in the _sqlx_migrations
    // table, and applies only unapplied migrations in version order.  This
    // replaces the previous sqlx::raw_sql approach which re-executed the full
    // schema on every startup and could not handle incremental schema changes
    // without data loss risk.
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("run database migrations")?;

    info!("database migrations applied successfully");

    // Spawn ingestion loop
    let ingest_pool = pool.clone();
    tokio::spawn(async move { ingest(ingest_pool, cfg).await });

    // REST API
    let app = Router::new()
        .route("/events", get(list_events))
        .route("/events/proposals/{id}", get(list_proposal_events))
        .with_state(pool);

    let addr = "0.0.0.0:4000";
    let listener = TcpListener::bind(addr).await?;
    info!("VoteChain indexer API on {addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Integration tests
// ---------------------------------------------------------------------------
//
// These tests spin up a real PostgreSQL instance via `testcontainers` (Docker
// required) and an in-process mock Horizon server, then drive `poll_once`.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use testcontainers_modules::{
        postgres::Postgres,
        testcontainers::{runners::AsyncRunner, ContainerAsync},
    };
    use tracing_test::traced_test;

    const CONTRACT: &str = "CTESTCONTRACT";

    struct Harness {
        _pg: ContainerAsync<Postgres>,
        pool: PgPool,
        cfg: Config,
        page: Arc<Mutex<Value>>,
        client: Client,
    }

    async fn harness() -> Harness {
        let pg = Postgres::default().start().await.expect("start postgres");
        let port = pg.get_host_port_ipv4(5432).await.expect("pg port");
        let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .connect(&url)
            .await
            .expect("connect");
        sqlx::migrate!("./migrations").run(&pool).await.expect("migrate");

        let page = Arc::new(Mutex::new(events_page(vec![])));
        let served = page.clone();
        let app = Router::new().route(
            "/contracts/{id}/events",
            get(move || {
                let served = served.clone();
                async move { Json(served.lock().unwrap().clone()) }
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let cfg = Config {
            database_url: url,
            horizon_url: format!("http://{addr}"),
            allowed_horizon_domains: vec![],
            contract_id: CONTRACT.into(),
            poll_interval: Duration::from_millis(10),
            backend_url: None,
        };
        Harness { _pg: pg, pool, cfg, page, client: Client::new() }
    }

    fn event(ledger: u64, tx: &str, topic: &str, proposal: i64) -> Value {
        serde_json::json!({
            "ledger": ledger,
            "transaction_hash": tx,
            "contract_id": CONTRACT,
            "topic": [topic, proposal],
            "value": { "tx": tx },
        })
    }

    fn events_page(records: Vec<Value>) -> Value {
        serde_json::json!({ "_embedded": { "records": records } })
    }

    impl Harness {
        fn serve(&self, records: Vec<Value>) {
            *self.page.lock().unwrap() = events_page(records);
        }

        async fn poll(&self) -> Result<usize> {
            poll_once(&self.client, &self.pool, &self.cfg).await
        }

        async fn rows(&self) -> Vec<(i64, String, String, Option<i64>)> {
            sqlx::query_as(
                "SELECT ledger_seq, tx_hash, topic, proposal_id
                 FROM contract_events ORDER BY ledger_seq, tx_hash",
            )
            .fetch_all(&self.pool)
            .await
            .unwrap()
        }

        async fn cursor(&self) -> u64 {
            last_ledger(&self.pool, CONTRACT).await.unwrap()
        }
    }

    #[tokio::test]
    async fn ingests_events_from_horizon_into_db() {
        let h = harness().await;
        h.serve(vec![
            event(10, "tx-a", "created", 1),
            event(11, "tx-b", "vote", 1),
            event(12, "tx-c", "final", 1),
        ]);

        assert_eq!(h.poll().await.unwrap(), 3);
        assert_eq!(
            h.rows().await,
            vec![
                (10, "tx-a".into(), "created".into(), Some(1)),
                (11, "tx-b".into(), "vote".into(), Some(1)),
                (12, "tx-c".into(), "final".into(), Some(1)),
            ]
        );
        assert_eq!(h.cursor().await, 12);
    }

    #[tokio::test]
    #[traced_test]
    async fn unknown_topic_is_skipped_and_logged() {
        let h = harness().await;
        h.serve(vec![
            event(20, "tx-a", "created", 1),
            event(21, "tx-b", "mystery", 1),
        ]);

        assert_eq!(h.poll().await.unwrap(), 1);
        let rows = h.rows().await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].2, "created");
        assert!(logs_contain("unknown event topic"));
        assert!(logs_contain("mystery"));
        // Skipped events still advance the cursor past their ledger.
        assert_eq!(h.cursor().await, 21);
    }

    #[tokio::test]
    async fn duplicate_events_are_deduplicated() {
        let h = harness().await;
        let batch = vec![event(30, "tx-a", "vote", 2), event(31, "tx-b", "vote", 2)];

        h.serve(batch.clone());
        h.poll().await.unwrap();
        // Horizon re-delivers the same events (e.g. cursor overlap on restart).
        h.serve([batch.clone(), batch].concat());
        h.poll().await.unwrap();

        assert_eq!(h.rows().await.len(), 2);
    }

    #[tokio::test]
    async fn cursor_does_not_advance_when_batch_fails_midway() {
        let h = harness().await;
        sqlx::query(
            "ALTER TABLE contract_events ADD CONSTRAINT test_reject_poison CHECK (tx_hash <> 'poison')",
        )
        .execute(&h.pool)
        .await
        .unwrap();

        h.serve(vec![
            event(40, "tx-a", "created", 3),
            event(41, "poison", "vote", 3),
            event(42, "tx-c", "final", 3),
        ]);

        assert!(h.poll().await.is_err());
        assert_eq!(h.cursor().await, 0, "cursor must not advance on a partial batch");
        assert!(h.rows().await.iter().all(|r| r.0 < 42), "later events not processed");

        // Once the failure clears, the batch is retried and the cursor advances.
        sqlx::query("ALTER TABLE contract_events DROP CONSTRAINT test_reject_poison")
            .execute(&h.pool)
            .await
            .unwrap();
        h.poll().await.unwrap();
        assert_eq!(h.cursor().await, 42);
        assert_eq!(h.rows().await.len(), 3);
    }
}

// ---------------------------------------------------------------------------
// Unit tests for validate_horizon_url (no Docker / async required)
// ---------------------------------------------------------------------------
#[cfg(test)]
mod ssrf_tests {
    use super::validate_horizon_url;

    fn domains(list: &str) -> Vec<String> {
        list.split(',').map(|s| s.trim().to_string()).collect()
    }

    fn default_domains() -> Vec<String> {
        domains("horizon-testnet.stellar.org,horizon.stellar.org")
    }

    // ---- scheme checks ----

    #[test]
    fn rejects_http_scheme() {
        let err = validate_horizon_url(
            "http://horizon-testnet.stellar.org",
            &default_domains(),
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("https://"),
            "error should mention https: {err}"
        );
    }

    #[test]
    fn rejects_ftp_scheme() {
        let err = validate_horizon_url(
            "ftp://horizon-testnet.stellar.org",
            &default_domains(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("https://"));
    }

    #[test]
    fn accepts_https_default_testnet() {
        validate_horizon_url(
            "https://horizon-testnet.stellar.org",
            &default_domains(),
        )
        .expect("valid testnet URL should be accepted");
    }

    #[test]
    fn accepts_https_default_mainnet() {
        validate_horizon_url(
            "https://horizon.stellar.org",
            &default_domains(),
        )
        .expect("valid mainnet URL should be accepted");
    }

    // ---- allowlist checks ----

    #[test]
    fn rejects_domain_not_in_allowlist() {
        let err = validate_horizon_url(
            "https://evil.example.com",
            &default_domains(),
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("not in the allowed domains list"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn accepts_custom_allowlist_domain() {
        validate_horizon_url(
            "https://my-horizon.internal.corp",
            &domains("my-horizon.internal.corp"),
        )
        .expect("custom allowlist domain should be accepted");
    }

    #[test]
    fn allowlist_check_is_case_insensitive() {
        validate_horizon_url(
            "https://HORIZON-TESTNET.STELLAR.ORG",
            &default_domains(),
        )
        .expect("case-insensitive match should succeed");
    }

    // ---- private IPv4 checks ----

    #[test]
    fn rejects_rfc1918_10_block() {
        // 10.0.0.0/8 in allowlist so domain check passes; IP check must fire.
        let err = validate_horizon_url(
            "https://10.1.2.3",
            &domains("10.1.2.3"),
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("private/loopback"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn rejects_rfc1918_172_16_block() {
        let err = validate_horizon_url(
            "https://172.16.0.1",
            &domains("172.16.0.1"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("private/loopback"));
    }

    #[test]
    fn rejects_rfc1918_172_31_block() {
        let err = validate_horizon_url(
            "https://172.31.255.255",
            &domains("172.31.255.255"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("private/loopback"));
    }

    #[test]
    fn rejects_rfc1918_192_168_block() {
        let err = validate_horizon_url(
            "https://192.168.1.1",
            &domains("192.168.1.1"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("private/loopback"));
    }

    #[test]
    fn rejects_loopback_127() {
        let err = validate_horizon_url(
            "https://127.0.0.1",
            &domains("127.0.0.1"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("private/loopback"));
    }

    #[test]
    fn rejects_link_local_169_254() {
        let err = validate_horizon_url(
            "https://169.254.1.1",
            &domains("169.254.1.1"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("private/loopback"));
    }

    #[test]
    fn accepts_public_ipv4_in_allowlist() {
        // 1.1.1.1 is a public Cloudflare DNS IP — should pass if allowlisted
        validate_horizon_url(
            "https://1.1.1.1",
            &domains("1.1.1.1"),
        )
        .expect("public IP in allowlist should be accepted");
    }

    // ---- private IPv6 checks ----

    #[test]
    fn rejects_ipv6_loopback() {
        let err = validate_horizon_url(
            "https://[::1]",
            &domains("::1"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("private/loopback"));
    }

    #[test]
    fn rejects_ipv6_unique_local_fc00() {
        let err = validate_horizon_url(
            "https://[fc00::1]",
            &domains("fc00::1"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("private/loopback"));
    }

    #[test]
    fn rejects_ipv6_unique_local_fd00() {
        let err = validate_horizon_url(
            "https://[fd00::1]",
            &domains("fd00::1"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("private/loopback"));
    }

    #[test]
    fn rejects_ipv6_link_local() {
        let err = validate_horizon_url(
            "https://[fe80::1]",
            &domains("fe80::1"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("private/loopback"));
    }

    // ---- malformed URL ----

    #[test]
    fn rejects_invalid_url() {
        let err = validate_horizon_url("not a url at all", &default_domains()).unwrap_err();
        assert!(err.to_string().contains("not a valid URL"));
    }
}
