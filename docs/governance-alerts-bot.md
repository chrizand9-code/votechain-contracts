# Governance Alerts Bot — Operator Manual

An optional bot that posts VoteChain governance events to Discord and/or
Telegram. It is **not** part of `docker-compose.yml` and must be run separately.

At least one notification channel must be configured — Discord, Telegram, or
both. When both are set, alerts are delivered to every channel in parallel so
a slow response from one provider never delays the others.

Source: [`bots/governance-alerts`](../bots/governance-alerts)

---

## Table of Contents

1. [Overview](#overview)
2. [Configuration Reference](#configuration-reference)
3. [Alert Types and Example Payloads](#alert-types-and-example-payloads)
4. [Step-by-Step Deployment](#step-by-step-deployment)
   - [Option A — VPS (systemd)](#option-a--vps-systemd)
   - [Option B — Cloud Function (AWS Lambda)](#option-b--cloud-function-aws-lambda)
   - [Option C — Docker](#option-c--docker)
5. [Log Interpretation](#log-interpretation)
6. [Restart Procedures](#restart-procedures)
7. [Troubleshooting](#troubleshooting)
8. [Extending to Other Channels](#extending-to-other-channels)

---

## Overview

The bot polls `GET {API_URL}/proposals` on a configurable interval. On the first poll
it seeds an in-memory cache of all existing proposals so they are **not** re-announced
on start. Subsequent polls detect state transitions and post a Discord message for each
significant change.

```
 ┌────────────┐   HTTP GET /proposals   ┌─────────────────┐   Webhook POST   ┌─────────┐
 │  Bot proc  │ ──────────────────────► │  VoteChain API  │                  │ Discord │
 │            │ ◄────────────────────── │  (backend)      │                  │ channel │
 │            │   JSON proposal list    └─────────────────┘                  │         │
 │            │ ─────────────────────────────────────────────────────────── ►│         │
 │            │                                                               └─────────┘
 │            │   Bot API POST    ┌─────────────────┐
 │            │ ────────────────► │  Telegram API  │ ──►  Telegram chat
 └────────────┘                   └─────────────────┘
```

**Requirements:** Node.js ≥ 18, npm, network access to the API and at least one
notification provider (Discord and/or Telegram).

---

## Configuration Reference

All configuration is supplied via environment variables. **Never commit secrets to
version control.**

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `DISCORD_WEBHOOK_URL` | no\* | — | Discord incoming webhook URL. Obtain from **Channel settings → Integrations → Webhooks → New Webhook**. Treat as a secret. |
| `TELEGRAM_BOT_TOKEN` | no\* | — | Telegram Bot API token. Obtain from [@BotFather](https://t.me/BotFather) by sending `/newbot`. Treat as a secret. |
| `TELEGRAM_CHAT_ID` | no\* | — | Telegram chat or channel ID to send alerts to. For channels, use the `@channelusername` or numeric ID (e.g. `-1001234567890`). |
| `API_URL` | no | `http://localhost:3001` | Base URL of the VoteChain backend API (no trailing slash). |
| `FRONTEND_URL` | no | `http://localhost:3000` | Base URL of the VoteChain frontend. Used to construct proposal deep-links included in alerts. |
| `POLL_INTERVAL_MS` | no | `30000` | How often (in milliseconds) to poll `GET /proposals`. Minimum recommended value is `10000` (10 s). Setting this below 5 s may cause rate-limiting on the API. |

\* At least one channel must be configured: `DISCORD_WEBHOOK_URL` for Discord,
or both `TELEGRAM_BOT_TOKEN` and `TELEGRAM_CHAT_ID` for Telegram. The bot exits
at startup if no channel is configured.

### Example `.env` file (never commit this)

```dotenv
# Discord (optional)
DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/123456789/AbCdEfGhIjKlMnOpQrStUvWx

# Telegram (optional)
TELEGRAM_BOT_TOKEN=123456789:ABCdefGHIjklMNOpqrsTUVwxyz
TELEGRAM_CHAT_ID=-1001234567890

# Shared
API_URL=https://api.votechain.example.com
FRONTEND_URL=https://app.votechain.example.com
POLL_INTERVAL_MS=30000
```

---

## Alert Types and Example Payloads

The bot sends plain-text messages to Discord via the incoming webhook. Each message
type corresponds to a governance lifecycle event detected by comparing the current
proposal state to the previously-observed state.

### `ProposalCreated`

Fires when a proposal ID is seen for the first time **after** the initial seed poll.

**Trigger:** `prev === undefined` (new proposal key in the cache)

**Discord message:**

```
🗳 **New proposal created**
**#42** — Increase quorum threshold to 10%
State: `active`
https://app.votechain.example.com/proposals/42
```

**Webhook body sent to Discord:**

```json
{
  "content": "🗳 **New proposal created**\n**#42** — Increase quorum threshold to 10%\nState: `active`\nhttps://app.votechain.example.com/proposals/42"
}
```

**Telegram message (Markdown):**

```
🗳️ *New proposal created*
*#42* — Increase quorum threshold to 10%
State: `active`
[View proposal](https://app.votechain.example.com/proposals/42)
```

**Bot API body sent to Telegram:**

```json
{
  "chat_id": "-1001234567890",
  "text": "🗳️ *New proposal created*\n*#42* — Increase quorum threshold to 10%\nState: `active`\n[View proposal](https://app.votechain.example.com/proposals/42)",
  "parse_mode": "Markdown",
  "disable_web_page_preview": true
}
```

---

### `ProposalFinalised`

Fires when a proposal transitions to a terminal state that is **not** `executed`.

**Terminal states:** `passed`, `rejected`, `failed`, `defeated`, `succeeded`,
`cancelled`, `expired`

**Trigger:** `FINAL_STATES.has(currentState) && prev !== currentState`

**Discord message:**

```
✅ **Proposal finalised**
**#42** — Increase quorum threshold to 10%
State: `rejected`
https://app.votechain.example.com/proposals/42
```

**Webhook body sent to Discord:**

```json
{
  "content": "✅ **Proposal finalised**\n**#42** — Increase quorum threshold to 10%\nState: `rejected`\nhttps://app.votechain.example.com/proposals/42"
}
```

**Telegram message (Markdown):**

```
✅ *Proposal finalised*
*#42* — Increase quorum threshold to 10%
State: `rejected`
[View proposal](https://app.votechain.example.com/proposals/42)
```

---

### `ProposalExecuted`

Fires when the `executed` flag on a proposal becomes `true`, or the state string
is `"executed"`.

**Trigger:** `stateOf(p) === "executed" && prev !== "executed"`

**Discord message:**

```
🚀 **Proposal executed**
**#42** — Increase quorum threshold to 10%
State: `executed`
https://app.votechain.example.com/proposals/42
```

**Webhook body sent to Discord:**

```json
{
  "content": "🚀 **Proposal executed**\n**#42** — Increase quorum threshold to 10%\nState: `executed`\nhttps://app.votechain.example.com/proposals/42"
}
```

**Telegram message (Markdown):**

```
🚀 *Proposal executed*
*#42* — Increase quorum threshold to 10%
State: `executed`
[View proposal](https://app.votechain.example.com/proposals/42)
```

---

## Step-by-Step Deployment

### Prerequisites (all options)

```bash
# 1. Clone or pull the repository
git clone https://github.com/veracindarella/votechain-contracts.git
cd votechain-contracts/bots/governance-alerts

# 2. Install Node.js dependencies
npm install

# 3. Compile TypeScript → JavaScript
npm run build
# Output: dist/index.js
```

---

### Option A — VPS (systemd)

Use this when running the bot on a Linux VPS (Ubuntu, Debian, etc.).

**1. Create a dedicated system user (optional but recommended):**

```bash
sudo useradd --system --shell /usr/sbin/nologin votechain-bot
```

**2. Copy the build output to `/opt/votechain-bot`:**

```bash
sudo mkdir -p /opt/votechain-bot
sudo cp -r dist node_modules package.json /opt/votechain-bot/
sudo chown -R votechain-bot:votechain-bot /opt/votechain-bot
```

**3. Create a secrets file at `/etc/votechain-bot.env` (mode 0600):**

```bash
sudo tee /etc/votechain-bot.env > /dev/null <<'EOF'
DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/YOUR_WEBHOOK_URL
TELEGRAM_BOT_TOKEN=123456789:ABCdefGHIjklMNOpqrsTUVwxyz
TELEGRAM_CHAT_ID=-1001234567890
API_URL=https://api.votechain.example.com
FRONTEND_URL=https://app.votechain.example.com
POLL_INTERVAL_MS=30000
EOF
sudo chmod 600 /etc/votechain-bot.env
sudo chown votechain-bot:votechain-bot /etc/votechain-bot.env
```

**4. Create the systemd unit at `/etc/systemd/system/votechain-bot.service`:**

```ini
[Unit]
Description=VoteChain Governance Alerts Bot
After=network.target

[Service]
Type=simple
User=votechain-bot
EnvironmentFile=/etc/votechain-bot.env
WorkingDirectory=/opt/votechain-bot
ExecStart=/usr/bin/node dist/index.js
Restart=on-failure
RestartSec=10
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

**5. Enable and start:**

```bash
sudo systemctl daemon-reload
sudo systemctl enable votechain-bot
sudo systemctl start votechain-bot
sudo systemctl status votechain-bot
```

**6. View live logs:**

```bash
journalctl -u votechain-bot -f
```

---

### Option B — Cloud Function (AWS Lambda)

Use this for a serverless, zero-maintenance deployment. Note that Lambda has a
maximum execution duration (15 minutes); the bot's infinite polling loop is
replaced by a scheduled invocation.

**1. Wrap the single `tick()` call in a Lambda handler:**

Create `lambda.ts` alongside `index.ts`:

```typescript
import { tick } from "./index";   // export tick() from index.ts first

export const handler = async (): Promise<void> => {
  await tick();
};
```

**2. Bundle with esbuild:**

```bash
npx esbuild lambda.ts --bundle --platform=node --target=node18 --outfile=dist/lambda.js
zip -j function.zip dist/lambda.js
```

**3. Deploy:**

```bash
aws lambda create-function \
  --function-name votechain-governance-alerts \
  --runtime nodejs18.x \
  --handler lambda.handler \
  --zip-file fileb://function.zip \
  --role arn:aws:iam::ACCOUNT_ID:role/lambda-basic-execution

# Set environment variables
aws lambda update-function-configuration \
  --function-name votechain-governance-alerts \
  --environment "Variables={DISCORD_WEBHOOK_URL=...,TELEGRAM_BOT_TOKEN=...,TELEGRAM_CHAT_ID=...,API_URL=...,FRONTEND_URL=...}"
```

**4. Add an EventBridge (CloudWatch Events) rule to trigger every 30 seconds:**

```bash
aws events put-rule \
  --name VotechainBotSchedule \
  --schedule-expression "rate(1 minute)"

aws events put-targets \
  --rule VotechainBotSchedule \
  --targets "Id=1,Arn=arn:aws:lambda:REGION:ACCOUNT_ID:function:votechain-governance-alerts"
```

> **Important:** Because each Lambda invocation is stateless, the in-memory
> `seen` cache is reset every invocation. Store state in DynamoDB or SSM
> Parameter Store if you want cross-invocation deduplication.

---

### Option C — Docker

**1. Create a `Dockerfile` inside `bots/governance-alerts/`:**

```dockerfile
FROM node:18-alpine
WORKDIR /app
COPY package.json .
RUN npm install --omit=dev
COPY dist/ dist/
CMD ["node", "dist/index.js"]
```

**2. Build and run:**

```bash
# From repo root
cd bots/governance-alerts
npm run build
docker build -t votechain-governance-alerts .
docker run -d \
  --name votechain-bot \
  --restart unless-stopped \
  -e DISCORD_WEBHOOK_URL="$DISCORD_WEBHOOK_URL" \
  -e TELEGRAM_BOT_TOKEN="$TELEGRAM_BOT_TOKEN" \
  -e TELEGRAM_CHAT_ID="$TELEGRAM_CHAT_ID" \
  -e API_URL="https://api.votechain.example.com" \
  -e FRONTEND_URL="https://app.votechain.example.com" \
  -e POLL_INTERVAL_MS=30000 \
  votechain-governance-alerts
```

**3. View logs:**

```bash
docker logs -f votechain-bot
```

---

## Log Interpretation

The bot writes structured messages to stdout/stderr. All lines are prefixed with
the Node.js process timestamp when piped through journald or a log aggregator.

| Log line | Meaning |
|----------|---------|
| `Governance alerts bot polling https://... every 30000ms` | Startup confirmation. Verify the URL and interval are correct. |
| `Poll failed: Error: API returned 503` | Transient API unavailability. The bot retries on the next interval; no manual action needed unless this persists. |
| `Poll failed: FetchError: ECONNREFUSED` | The API host is unreachable. Check `API_URL` and network/firewall rules. |
| `Discord webhook failed: 401 ...` | Webhook URL is invalid or was revoked. Regenerate the webhook in Discord and update `DISCORD_WEBHOOK_URL`. |
| `Discord webhook failed: 429 ...` | Discord rate-limit hit. Increase `POLL_INTERVAL_MS` to reduce posting frequency. |
| `Telegram post failed: 401 ...` | Bot token is invalid. Re-create the token via @BotFather and update `TELEGRAM_BOT_TOKEN`. |
| `Telegram post failed: 400 ...` | `TELEGRAM_CHAT_ID` is wrong or the bot has not been added to the target chat/channel. |
| `No notification channel configured ...` | Bot exited at startup — no channel configured. Set at least one of `DISCORD_WEBHOOK_URL` or `TELEGRAM_BOT_TOKEN` + `TELEGRAM_CHAT_ID`. |

---

## Restart Procedures

### systemd

```bash
# Graceful restart (picks up env file changes)
sudo systemctl restart votechain-bot

# Hard stop then start
sudo systemctl stop votechain-bot
sudo systemctl start votechain-bot

# Check status after restart
sudo systemctl status votechain-bot
```

> **Note on cache warm-up:** Every restart resets the in-memory `seen` cache.
> On the first poll after a restart, the bot re-seeds the cache from the current
> proposal list without posting alerts. Proposals that changed state *during*
> the downtime will not generate alerts. This is intentional to prevent alert
> storms after maintenance windows.

### Docker

```bash
docker restart votechain-bot
```

### AWS Lambda

Redeploy the function with an updated zip to pick up code changes:

```bash
zip -j function.zip dist/lambda.js
aws lambda update-function-code \
  --function-name votechain-governance-alerts \
  --zip-file fileb://function.zip
```

---

## Troubleshooting

### Bot starts but no alerts appear in Discord

1. Confirm `DISCORD_WEBHOOK_URL` is set and correct:
   ```bash
   # Quick smoke test
   curl -H "Content-Type: application/json" \
        -d '{"content":"VoteChain bot test"}' \
        "$DISCORD_WEBHOOK_URL"
   # Expected: HTTP 204
   ```
2. Check that the bot is past its first seed poll (look for the startup log line).
3. Verify `API_URL` returns proposals:
   ```bash
   curl "$API_URL/proposals"
   ```
4. Create a new proposal on the governance contract and wait one poll interval.

### Bot floods Discord with duplicate alerts

- Likely caused by repeated restarts in a short window or a redeployment that
  did not wait for the cache warm-up poll. Set `POLL_INTERVAL_MS` higher, or
  implement a persistent state store (Redis / DynamoDB) if you need deduplication
  across restarts.

### `Poll failed: invalid json response body`

The API returned non-JSON (e.g., an HTML error page). Check the API server logs
and confirm the backend is healthy.

### Bot crashes on startup with `Cannot find module 'dist/index.js'`

TypeScript was not compiled. Run `npm run build` before starting.

### High memory usage over time

The `seen` Map grows linearly with the number of unique proposals. This is
negligible for typical governance use (thousands of proposals), but if you observe
memory growth, restart the bot periodically during maintenance windows.

---

## Extending to Other Channels

### Telegram (built-in since issue #323)

Telegram is supported natively alongside Discord. Set `TELEGRAM_BOT_TOKEN`
and `TELEGRAM_CHAT_ID` and the bot will post to both channels in parallel.
Messages are formatted with Telegram Markdown (`parse_mode=Markdown`), using
single-asterisk bold (`*text*`) and Markdown links (`[text](url)`) rather
than the Discord double-asterisk syntax.

See [Configuration Reference](#configuration-reference) above for details.

### Slack

Use the Slack incoming webhooks API (identical shape to Discord):

```typescript
async function post(content: string): Promise<void> {
  const res = await fetch(process.env.SLACK_WEBHOOK_URL as string, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ text: content }),
  });
  if (!res.ok) console.error(`Slack post failed: ${res.status}`);
}
```

---

## Related Documentation

- [`docs/governance-alerts-bot.md`](governance-alerts-bot.md) — this file
- [`bots/governance-alerts/index.ts`](../bots/governance-alerts/index.ts) — bot source code
- [`docs/api-reference.md`](api-reference.md) — API endpoints polled by the bot
- [`docs/GETTING_STARTED.md`](GETTING_STARTED.md) — general environment setup
