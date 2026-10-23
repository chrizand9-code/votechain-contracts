# Observability

The VoteChain backend (`backend/`) exposes Prometheus metrics and ships with a
local Prometheus + Grafana stack for development. (#66)

## Kubernetes health probes

`GET /live` is a liveness probe: it returns HTTP 200 while the process can
serve requests and does not depend on external services. `GET /ready` checks
PostgreSQL, Redis, and Stellar RPC and returns HTTP 503 if any component is
degraded. `GET /health` returns the same readiness status plus the per-component
results for diagnostics.

Set `HEALTH_CHECK_TIMEOUT_MS` to a positive number of milliseconds to change
the per-component timeout (default: 2000 ms). Configure `DATABASE_URL`,
`REDIS_URL`, and `STELLAR_RPC_URL` for the readiness checks.

Use `/live` for Kubernetes `livenessProbe` and `/ready` for
`readinessProbe`; do not use `/health` as the liveness probe because a
dependency outage should not cause Kubernetes to repeatedly restart a healthy
process.

```yaml
livenessProbe:
  httpGet:
    path: /live
    port: 3001
readinessProbe:
  httpGet:
    path: /ready
    port: 3001
```

## Metrics endpoint

`GET /metrics` returns metrics in Prometheus text-exposition format, mounted
before rate-limiting and auth (same rationale as `/health` and `/ready` — see
[STAGING.md](STAGING.md) and `backend/src/routes/health.ts`).

| Metric | Type | Labels | Description |
|---|---|---|---|
| `votechain_http_requests_total` | counter | `method`, `route`, `status` | Total HTTP requests |
| `votechain_http_request_duration_seconds` | histogram | `method`, `route`, `status` | Request latency; use `histogram_quantile()` for p50/p95/p99 |
| `votechain_cache_hit_rate` | gauge | — | Redis proposal-cache hit rate (0–1), backed by the existing `GET /metrics/cache` counters |
| `votechain_active_db_connections` | gauge | — | 1 when Redis (the backend's only persistent-connection dependency today) is connected, 0 otherwise |

Route labels use the matched Express route pattern (e.g. `/api/proposals/:id`),
not the raw URL, to keep cardinality bounded.

Default Node.js process metrics (`process_cpu_seconds_total`, memory,
event-loop lag, etc.) are also collected via `prom-client`'s
`collectDefaultMetrics()`.

## Local stack

```bash
docker compose up backend redis prometheus grafana
```

- Prometheus: http://localhost:9090 (scrapes `backend:3001/metrics` every 15s,
  config in [`infra/prometheus/prometheus.yml`](../infra/prometheus/prometheus.yml))
- Grafana: http://localhost:3002 (admin/admin, anonymous viewer access enabled
  for local dev) — the "VoteChain Backend" dashboard is auto-provisioned from
  [`infra/grafana/votechain-backend.json`](../infra/grafana/votechain-backend.json)
  via [`infra/grafana/provisioning`](../infra/grafana/provisioning)

The dashboard includes: request rate by route/status, p50/p95/p99 latency,
cache hit rate, active DB (Redis) connections, and 5xx error ratio.

### SLO dashboard (#272)

A dedicated SLO dashboard is available at
[`infra/grafana/votechain-api-slo.json`](../infra/grafana/votechain-api-slo.json),
auto-provisioned alongside the backend dashboard. It tracks two SLOs:

| SLO | Target | Measurement |
|-----|--------|-------------|
| Availability | 99.9% (error budget = 0.1%) | 5xx error rate vs total requests |
| Latency | P95 < 500ms | 95th percentile request duration |

Panels:

- **Availability %** — current and historical availability with SLO threshold
- **Error Budget Remaining** — gauge showing how much of the 30-day error
  budget remains
- **Error Budget Burn Rate** — multi-window burn rate (5m, 1h, 6h) with 2x
  and 5x threshold lines
- **P95 Latency vs SLO** — P95 latency with the 500ms SLO line
- **Error Rate** — 5xx error rate with the 0.1% error budget line
- **Request Rate by Status Class** — stacked request rate by HTTP status
- **Latency Percentiles** — p50/p95/p99 with the 500ms SLO line

## Alerting

Alert rules live in
[`infra/prometheus/alerts.yml`](../infra/prometheus/alerts.yml) and are loaded
by Prometheus via `rule_files`:

- **HighApiLatencyP99** — fires when p99 request latency exceeds 2s for 5
  minutes.
- **LowCacheHitRate** — fires when the Redis cache hit rate drops below 50%
  for 10 minutes.
- **SLOErrorBudgetBurnRate2x** — fires when the 5xx error rate exceeds 2x
  the error budget allowance over both 1h and 5m windows (warning).
- **SLOErrorBudgetBurnRate5x** — fires when the 5xx error rate exceeds 5x
  the error budget allowance over both 6h and 30m windows (critical).
- **SLOLatencyP95Exceeded** — fires when P95 latency exceeds 500ms for 10
  minutes.

These rules define alert *conditions* only; wiring Alertmanager to a paging
or chat channel is left to the deploying environment and is not included
here.

## Known gaps

- `votechain_active_db_connections` reflects Redis connectivity (the only
  persistent-connection dependency the Node backend currently has), not a
  SQL connection pool. The `indexer` service maintains a Postgres pool
  (`sqlx::PgPool`) but does not yet expose Prometheus metrics — a natural
  follow-up if/when the indexer's HTTP surface needs its own dashboard.
- Grafana runs with anonymous viewer access and a default `admin/admin`
  password for local-dev convenience only; do not reuse this
  `docker-compose.yml` configuration as-is in a shared or public environment.

## Log rotation and retention (#269)

### Docker Compose log rotation

All services in `docker-compose.yml` share a common `x-logging` YAML anchor
that enforces the `json-file` driver with bounded rotation:

```yaml
x-logging: &default-logging
  driver: json-file
  options:
    max-size: "50m"   # rotate when a log file reaches 50 MB
    max-file: "5"     # keep at most 5 rotated files per container (≤ 250 MB total)
```

This means the maximum on-disk footprint per container is **250 MB**
(5 × 50 MB).  Older files are silently removed by the Docker daemon as new
ones are created.

Every service (`dev`, `backend`, `redis`, `stellar-node`, `prometheus`,
`grafana`) references this anchor via `logging: *default-logging`, so the
policy is applied uniformly.

### Production log shipping

For production deployments it is strongly recommended to ship logs to an
external aggregator rather than relying solely on the `json-file` driver.
The aggregator endpoint is configured via environment variables:

| Variable | Description | Example |
|---|---|---|
| `LOG_AGGREGATOR_URL` | HTTP(S) endpoint of your log aggregator | `https://logs.example.com/ingest` |
| `LOG_AGGREGATOR_TOKEN` | Bearer token / API key for authentication | *(set via secret manager)* |
| `LOG_LEVEL` | Minimum log level forwarded (`debug`/`info`/`warn`/`error`) | `info` |

Supported aggregators include **Loki** (via Grafana Alloy or Promtail),
**Datadog**, **Elastic/OpenSearch**, and any service that accepts newline-
delimited JSON over HTTP.

To enable shipping at the Docker Compose level, replace (or supplement) the
`json-file` driver with the vendor-provided log driver.  Example for Loki:

```yaml
x-logging: &default-logging
  driver: loki
  options:
    loki-url: "${LOG_AGGREGATOR_URL}"
    loki-tenant-id: "votechain"
    max-size: "50m"
    max-file: "5"
```

### Sensitive-field redaction

The backend **must not** ship raw log lines that contain:

- API keys or bearer tokens (request headers `Authorization`, `X-Api-Key`)
- Admin secrets or private key material
- Full wallet addresses in error traces where not necessary

The existing `backend/src/middleware/` stack already omits these from
structured log output.  If you add a new logging call, ensure you redact
by excluding the field or replacing the value with `[REDACTED]` before
writing to `console.log` / the logger.

**Never** log the raw value of `process.env.ADMIN_SECRET`,
`process.env.API_KEY_*`, or any variable whose name contains `KEY`,
`SECRET`, or `TOKEN`.
