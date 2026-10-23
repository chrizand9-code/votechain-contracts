# Mainnet Deployment Runbook

> **Audience:** Release engineers and on-call engineers performing or reviewing a mainnet deployment.
> **Related:** [`scripts/deploy_mainnet.sh`](../scripts/deploy_mainnet.sh), [`.github/workflows/deploy-mainnet.yml`](../.github/workflows/deploy-mainnet.yml)

---

## Overview

VoteChain mainnet deployments follow a **build → staging → manual approval → mainnet** pipeline enforced by GitHub Actions and GitHub Environments.

```
Push version tag (v*.*.*)
        │
        ▼
┌──────────────────┐
│  Verify CI green │  Confirms tests passed on the exact commit being deployed
└──────────────────┘
        │
        ▼
┌──────────────────────────┐
│  Download CI WASM binary │  Reuses the artifact from CI — no re-build
└──────────────────────────┘
        │
        ▼
┌────────────────────────────┐
│  Deploy to staging (testnet)│  Real deploy + smoke tests against testnet
└────────────────────────────┘
        │
        ▼
┌─────────────────────────────────┐
│  Manual approval gate (mainnet  │  GitHub Environment: "mainnet" — requires
│  Environment required reviewers)│  N reviewers to approve via GitHub UI
└─────────────────────────────────┘
        │
        ▼
┌─────────────────────────────┐
│  Deploy to mainnet          │  Same WASM binary — no re-build
└─────────────────────────────┘
        │
        ▼
┌──────────────────────────────────┐
│  Update config/mainnet.toml      │  Contract addresses committed back to repo
│  Create GitHub Release           │  Release note with addresses + WASM hashes
└──────────────────────────────────┘
```

---

## Prerequisites

Before cutting a release tag, confirm all of the following:

| Check | Command / Location |
|-------|--------------------|
| All CI checks pass on `main` | GitHub Actions → CI workflow |
| Security audit clean | `cargo audit` in CI |
| WASM size within budget | CI build-wasm job |
| `CHANGELOG.md` updated | `CHANGELOG.md` |
| ADRs updated if architecture changed | `docs/adr/` |
| At least one reviewer available to approve | GitHub → Settings → Environments → mainnet |
| `MAINNET_SECRET_KEY` secret set | GitHub → Settings → Secrets → Actions |
| `MAINNET_ADMIN_ADDRESS` secret set | GitHub → Settings → Secrets → Actions |
| `STAGING_SECRET_KEY` secret set | GitHub → Settings → Secrets → Actions |
| `STAGING_ADMIN_ADDRESS` secret set | GitHub → Settings → Secrets → Actions |

---

## Step-by-Step Deployment

### 1. Create and push a version tag

```bash
# Ensure you are on the latest main
git checkout main
git pull origin main

# Tag follows semver: v<major>.<minor>.<patch>
git tag -a v1.2.3 -m "Release v1.2.3"
git push origin v1.2.3
```

This triggers the `deploy-mainnet.yml` workflow automatically.

### 2. Monitor the CI verification step

Open **GitHub Actions → Deploy to Mainnet** and watch the **Verify CI Green** job.

- The job polls for up to 10 minutes waiting for any in-progress CI runs on the same commit to finish.
- If CI has not passed it aborts with `❌ CI did not pass`.
- If CI passed it continues to the next job.

### 3. WASM artifact download

The **Download CI WASM Artifact** job locates the `wasm-contracts-*` artifact produced by the
CI `build-wasm` job for the exact commit being released.

> **Important:** No re-build occurs. The mainnet binary is byte-for-byte identical to what CI tested.

If the artifact cannot be found (e.g., the CI run expired after 90 days), the job fails with
`❌ No wasm-contracts artifact found`. In that case re-run CI on the tag's commit and retry.

### 4. Review the staging deploy

The **Deploy to Staging** job deploys to Stellar testnet using the `staging` GitHub Environment.

- Outputs: token contract ID and governance contract ID on testnet.
- Smoke tests call `total_supply` on the token contract to verify it is live.

Check the step logs before approving the mainnet gate.

### 5. Approve the mainnet gate

After staging passes, the workflow pauses at the **Await Mainnet Approval** job.

A notification appears in:
- GitHub Actions timeline (yellow dot on the `Await Mainnet Approval` job)
- Email/Slack if GitHub notification routing is configured for the `mainnet` Environment

**To approve:**
1. Go to **GitHub Actions → Deploy to Mainnet → the paused run**.
2. Click **Review deployments**.
3. Select the `mainnet` environment and click **Approve and deploy**.

> At least the number of required reviewers configured for the `mainnet` Environment must approve.
> The account that pushed the tag cannot self-approve (configured on the Environment).

### 6. Verify the mainnet deploy

After approval the **Deploy to Mainnet** job runs. Check the logs for:

```
✅ Token deployed: C...
✅ Governance deployed: C...
```

The contract IDs are also captured as job outputs and used in the release notes.

### 7. Confirm config/mainnet.toml commit

A bot commit appears on `main` updating `config/mainnet.toml` with the new contract addresses.
Verify the commit message:

```
chore: update mainnet contract addresses for v1.2.3 [skip ci]
```

### 8. Verify the GitHub Release

Navigate to **GitHub → Releases → v1.2.3** and confirm:

- Token and governance contract addresses are correct.
- WASM SHA-256 hashes are listed.
- Both WASM files are attached as release assets.

---

## Environment Configuration

### Required GitHub Environments

| Environment | Used by | Required reviewers |
|-------------|---------|-------------------|
| `staging` | `deploy-staging` job | None (automated) |
| `mainnet` | `await-approval` and `deploy-mainnet` jobs | **Set to ≥ 1 (recommend 2)** |

**Setting up the `mainnet` Environment:**
1. GitHub → Settings → Environments → New environment → `mainnet`
2. Enable **Required reviewers** and add the release engineers.
3. Enable **Prevent self-review**.
4. Optionally set **Deployment branch rules** to tags matching `v*.*.*` only.

### Required Secrets

| Secret | Environment | Description |
|--------|-------------|-------------|
| `MAINNET_SECRET_KEY` | `mainnet` | Stellar secret key for the deployer account |
| `MAINNET_ADMIN_ADDRESS` | `mainnet` | Stellar address that will be the contract admin |
| `STAGING_SECRET_KEY` | `staging` | Stellar secret key for staging deployer |
| `STAGING_ADMIN_ADDRESS` | `staging` | Stellar address for staging admin |

Set secrets at: **GitHub → Settings → Environments → \<env\> → Environment secrets**.

> **Security:** Never commit secret keys to the repository. Rotate keys immediately if exposed.

---

## Manual Deployment (Break-Glass)

If the GitHub Actions pipeline is unavailable, use `scripts/deploy_mainnet.sh` directly
after following all the pre-flight checks above.

```bash
# Dry-run first
STELLAR_SECRET_KEY=$KEY \
STELLAR_ADMIN_ADDRESS=$ADMIN \
./scripts/deploy_mainnet.sh --mainnet --dry-run

# Real deploy (requires interactive confirmation)
STELLAR_SECRET_KEY=$KEY \
STELLAR_ADMIN_ADDRESS=$ADMIN \
./scripts/deploy_mainnet.sh --mainnet
```

After a manual deploy, manually update `config/mainnet.toml` and create a GitHub Release.

---

## Admin Key Rotation

Rotating the governance contract admin is a separate operational procedure
covered in its own runbook:

> **[docs/admin-key-rotation-runbook.md](admin-key-rotation-runbook.md)**

That runbook covers: when to rotate, prerequisites, the two-step
propose/accept flow via `scripts/rotate-admin.sh`, on-chain verification,
and rollback procedures for each failure mode.

---

## Logging and Log Rotation

Production containers (`backend`, `indexer`) log structured JSON to stdout. Local `json-file`
rotation (`docker-compose.yml`, 100MB × 5 files per service) bounds host disk usage, but mainnet
deployments must also ship logs to a centralised backend (Loki or CloudWatch Logs) with **30-day
retention**. See [docs/logging.md](logging.md) for driver configuration and retention setup.

---

## Rollback

There is no on-chain contract rollback. Once deployed, a contract is immutable.

Mitigation options:

1. **Pause the contract** — Call `pause(admin)` to block all state-changing operations.
2. **Deploy a new version** — Push a new version tag to trigger a new deployment.
3. **Update config** — Point `config/mainnet.toml` to the new contract addresses.

---

## Troubleshooting

### `❌ No wasm-contracts artifact found`

The CI artifact for this commit has expired (90-day retention) or CI has not run.
- Re-run the CI workflow on the tag commit.
- Re-push the tag after CI completes: `git push origin v1.2.3 --force` (only if tag is wrong).

### `❌ CI did not pass`

Fix the failing CI check, push a new tag (e.g., `v1.2.4`), and restart the process.

### Stellar CLI errors during deploy

- Ensure `MAINNET_SECRET_KEY` is a funded mainnet account.
- Check RPC URL is reachable: `curl https://horizon.stellar.org`.
- Verify the deployer account has sufficient XLM for transaction fees.

### Approval gate not appearing

- Confirm the `mainnet` GitHub Environment exists and has required reviewers.
- Check the Actions run is not stuck on a prior step.

---

## Checklist — Post-Deploy

- [ ] Contract addresses in `config/mainnet.toml` are correct
- [ ] GitHub Release `v*.*.* ` created with addresses and WASM hashes
- [ ] CHANGELOG.md entry merged to `main`
- [ ] Announce in community channels
- [ ] Monitor contract events on [Stellar Expert](https://stellar.expert) for the first 24 hours

---

## Indexer Lag

> **Alert:** `IndexerLagHigh` — fires when `indexer_event_lag_seconds > 300` for 2 minutes.

The indexer consumes Soroban events emitted by the governance contract and writes them to
PostgreSQL. When it falls behind, the UI shows stale proposal states (e.g., proposals that
have finalised on-chain still appear as Active in the dashboard).

### What triggers the alert

The Prometheus metric `indexer_event_lag_seconds` measures the wall-clock delta between
the timestamp of the most recently finalised on-chain event and the timestamp when the
indexer last persisted it. When this exceeds **300 seconds (5 minutes)**, the
`IndexerLagHigh` alert fires at `critical` severity.

### Immediate response

1. **Check the indexer process:**
   ```bash
   # In the deployment environment
   systemctl status votechain-indexer   # or: docker compose logs indexer
   ```
2. **Look for errors in the logs** — common causes:
   - Lost connection to the Stellar RPC node (network partition, node restart)
   - PostgreSQL connection pool exhausted
   - Indexer OOM-killed or crashed with a panic

3. **Restart the indexer** (safe — it picks up from its last checkpoint):
   ```bash
   systemctl restart votechain-indexer   # or: docker compose restart indexer
   ```

4. **Verify recovery** — the metric should drop below 60s within 2–3 minutes of
   a successful restart. If it does not, escalate.

### If the indexer is healthy but lag persists

- **Check Stellar RPC node health:** `curl https://soroban-mainnet.stellar.org/` and compare
  `ledger_current` against [Stellar Expert](https://stellar.expert) to detect node lag.
- **Check database disk / connection limits** — a full disk or max connections error
  can cause silent write failures without crashing the process.

### Grafana dashboard

The **Indexer event lag (seconds)** panel on the
[VoteChain Backend dashboard](http://localhost:3002/d/votechain-backend) shows a
time-series of the lag metric with threshold lines at 120s (warning) and 300s (critical).
