// Copyright 2024 VoteChain Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

/**
 * VoteChain governance alerts bot.
 *
 * Polls the backend API for proposals and posts notifications to Discord
 * (via an incoming webhook) and/or Telegram (via the Bot API) when a
 * proposal is created, finalised, or executed.
 *
 * Environment:
 *   DISCORD_WEBHOOK_URL   (optional) Discord webhook URL — store as a secret
 *   TELEGRAM_BOT_TOKEN    (optional) Telegram Bot API token from @BotFather
 *   TELEGRAM_CHAT_ID      (optional) Telegram chat/channel ID to send alerts to
 *   API_URL               Backend base URL (default http://localhost:3001)
 *   FRONTEND_URL          Frontend base URL for deep links (default http://localhost:3000)
 *   POLL_INTERVAL_MS      Poll interval in ms (default 30000)
 *
 * At least one notification channel (Discord or Telegram) must be configured.
 */

interface Proposal {
  id: number | string;
  title?: string;
  state?: string;
  status?: string;
  executed?: boolean;
}

// ── Channel configuration ───────────────────────────────────────────────────

const DISCORD_WEBHOOK = process.env.DISCORD_WEBHOOK_URL;
const TELEGRAM_BOT_TOKEN = process.env.TELEGRAM_BOT_TOKEN;
const TELEGRAM_CHAT_ID = process.env.TELEGRAM_CHAT_ID;

const API_URL = (process.env.API_URL ?? "http://localhost:3001").replace(/\/$/, "");
const FRONTEND_URL = (process.env.FRONTEND_URL ?? "http://localhost:3000").replace(/\/$/, "");
const POLL_INTERVAL_MS = Number(process.env.POLL_INTERVAL_MS ?? 30000);

const FINAL_STATES = new Set([
  "passed",
  "rejected",
  "failed",
  "defeated",
  "succeeded",
  "cancelled",
  "expired",
]);

type EventKind = "ProposalCreated" | "ProposalFinalised" | "ProposalExecuted";

const seen = new Map<string, string>();
let initialised = false;

// ── Helpers ────────────────────────────────────────────────────────────────

function stateOf(p: Proposal): string {
  if (p.executed) return "executed";
  return String(p.state ?? p.status ?? "active").toLowerCase();
}

/** Whether at least one notification channel is configured. */
function hasChannel(): boolean {
  return Boolean(DISCORD_WEBHOOK) || Boolean(TELEGRAM_BOT_TOKEN && TELEGRAM_CHAT_ID);
}

// ── Message formatters ─────────────────────────────────────────────────────

/**
 * Formats a message for Discord using Discord markdown (`**bold**`, backtick
 * code spans).
 */
export function formatDiscordMessage(kind: EventKind, p: Proposal): string {
  const title = p.title ?? "(untitled)";
  const link = `${FRONTEND_URL}/proposals/${p.id}`;
  const headline: Record<EventKind, string> = {
    ProposalCreated: "🗳️ **New proposal created**",
    ProposalFinalised: "✅ **Proposal finalised**",
    ProposalExecuted: "🚀 **Proposal executed**",
  };
  return [
    headline[kind],
    `**#${p.id}** — ${title}`,
    `State: \`${stateOf(p)}\``,
    link,
  ].join("\n");
}

/**
 * Escapes characters that Telegram's Markdown parser treats as formatting
 * markers so that proposal titles display verbatim.
 */
function escapeTelegram(text: string): string {
  // In legacy Markdown, escape: *, _, `, [
  return text.replace(/([*_`\[])/g, "\\$1");
}

/**
 * Formats a message for Telegram using Telegram Markdown (`*bold*`,
 * `[text](url)`, backtick code spans).
 *
 * Telegram uses single-asterisk bold whereas Discord uses double-asterisk,
 * so a separate formatter is required.
 */
export function formatTelegramMessage(kind: EventKind, p: Proposal): string {
  const title = escapeTelegram(p.title ?? "(untitled)");
  const link = `${FRONTEND_URL}/proposals/${p.id}`;
  const state = stateOf(p);
  const headline: Record<EventKind, string> = {
    ProposalCreated: "🗳️ *New proposal created*",
    ProposalFinalised: "✅ *Proposal finalised*",
    ProposalExecuted: "🚀 *Proposal executed*",
  };
  return [
    headline[kind],
    `*#${p.id}* — ${title}`,
    `State: \`${state}\``,
    `[View proposal](${link})`,
  ].join("\n");
}

// ── Channel senders ────────────────────────────────────────────────────────

/** Posts a message to Discord via an incoming webhook. */
async function postDiscord(content: string): Promise<void> {
  if (!DISCORD_WEBHOOK) return;
  const res = await fetch(DISCORD_WEBHOOK, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ content }),
  });
  if (!res.ok)
    console.error(`Discord webhook failed: ${res.status} ${await res.text()}`);
}

/** Posts a message to a Telegram chat via the Bot API sendMessage endpoint. */
async function postTelegram(content: string): Promise<void> {
  if (!TELEGRAM_BOT_TOKEN || !TELEGRAM_CHAT_ID) return;
  const res = await fetch(
    `https://api.telegram.org/bot${TELEGRAM_BOT_TOKEN}/sendMessage`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        chat_id: TELEGRAM_CHAT_ID,
        text: content,
        parse_mode: "Markdown",
        disable_web_page_preview: true,
      }),
    },
  );
  if (!res.ok)
    console.error(
      `Telegram post failed: ${res.status} ${await res.text()}`,
    );
}

/**
 * Sends a notification to every configured channel.
 *
 * Channels fire in parallel so a slow response from one provider does not
 * delay the others.  Errors are logged but never thrown so a single channel
 * failure does not suppress delivery to the rest.
 */
async function notify(kind: EventKind, p: Proposal): Promise<void> {
  const tasks: Promise<void>[] = [];

  if (DISCORD_WEBHOOK) {
    tasks.push(postDiscord(formatDiscordMessage(kind, p)));
  }
  if (TELEGRAM_BOT_TOKEN && TELEGRAM_CHAT_ID) {
    tasks.push(postTelegram(formatTelegramMessage(kind, p)));
  }

  await Promise.allSettled(tasks);
}

// ── Proposal polling ───────────────────────────────────────────────────────

async function fetchProposals(): Promise<Proposal[]> {
  const res = await fetch(`${API_URL}/proposals`);
  if (!res.ok) throw new Error(`API returned ${res.status}`);
  const body = (await res.json()) as
    | Proposal[]
    | { data?: Proposal[]; proposals?: Proposal[] };
  if (Array.isArray(body)) return body;
  return body.data ?? body.proposals ?? [];
}

/** Exported for AWS Lambda single-invocation mode. */
export async function tick(): Promise<void> {
  const proposals = await fetchProposals();
  for (const p of proposals) {
    const key = String(p.id);
    const state = stateOf(p);
    const prev = seen.get(key);
    seen.set(key, state);
    // First poll seeds the cache so existing proposals are not re-announced.
    if (!initialised || prev === state) continue;
    if (prev === undefined) await notify("ProposalCreated", p);
    else if (state === "executed") await notify("ProposalExecuted", p);
    else if (FINAL_STATES.has(state)) await notify("ProposalFinalised", p);
  }
  initialised = true;
}

// ── Entry point ────────────────────────────────────────────────────────────

async function main(): Promise<void> {
  if (!hasChannel()) {
    console.error(
      "No notification channel configured. Set DISCORD_WEBHOOK_URL and/or TELEGRAM_BOT_TOKEN + TELEGRAM_CHAT_ID.",
    );
    process.exit(1);
  }

  const channels: string[] = [];
  if (DISCORD_WEBHOOK) channels.push("Discord");
  if (TELEGRAM_BOT_TOKEN && TELEGRAM_CHAT_ID) channels.push("Telegram");

  console.log(
    `Governance alerts bot polling ${API_URL}/proposals every ${POLL_INTERVAL_MS}ms → ${channels.join(", ")}`,
  );

  for (;;) {
    try {
      await tick();
    } catch (err) {
      console.error("Poll failed:", err);
    }
    await new Promise((r) => setTimeout(r, POLL_INTERVAL_MS));
  }
}

if (require.main === module) void main();
