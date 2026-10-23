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
 * Admin JWT authentication middleware for VoteChain backend API.
 *
 * Usage:
 *   import { adminAuth } from "./adminAuth";
 *   router.post("/proposals/invalidate", adminAuth, handler);
 *
 * Environment variables:
 *   ADMIN_JWT_SECRET            Secret used to verify tokens (default: "changeme")
 *   ADMIN_STELLAR_ADDRESS       The on-chain admin Stellar address that must appear in the token payload
 *   ADMIN_AUTH_RATE_LIMIT       Max auth attempts per IP per minute (default: 5) — #318
 *   ADMIN_AUTH_BLOCK_THRESHOLD  Failed attempts within the block window that trigger an IP block (default: 10) — #318
 *   ADMIN_AUTH_BLOCK_WINDOW_MS  Rolling window (ms) over which failed attempts are counted for blocking (default: 300000 = 5 min) — #318
 *   ADMIN_AUTH_BLOCK_DURATION_MS  How long an IP stays blocked once it trips the threshold (default: 300000 = 5 min) — #318
 *
 * Token format:
 *   JWT signed with HMAC-SHA256, payload: { stellarAddress: string }
 *   Passed via: Authorization: Bearer <token>
 *
 * Rate limiting (#318 — Security: rate limiting for authentication endpoints):
 *   - Max ADMIN_AUTH_RATE_LIMIT requests per minute per IP (in-memory, resets per window).
 *   - Every failed auth attempt (missing/invalid token, wrong admin address) is
 *     logged with the source IP and timestamp.
 *   - An IP that accumulates more than ADMIN_AUTH_BLOCK_THRESHOLD failed attempts
 *     within ADMIN_AUTH_BLOCK_WINDOW_MS is blocked for ADMIN_AUTH_BLOCK_DURATION_MS,
 *     independent of the per-minute rate limit above (brute-force defence).
 */

import { Request, Response, NextFunction } from "express";
import jwt from "jsonwebtoken";
import { log } from "./requestTracing";

// ── Types ──────────────────────────────────────────────────────────────────

interface AdminTokenPayload {
  stellarAddress: string;
}

interface RateLimitEntry {
  count: number;
  resetAt: number; // Unix epoch ms
}

interface FailureEntry {
  /** Timestamps (ms) of failed attempts still inside the block window. */
  timestamps: number[];
  /** Set when the IP is currently blocked; cleared once it expires. */
  blockedUntil: number | null;
}

// ── Configuration (env-driven, #318) ───────────────────────────────────────

function envInt(name: string, fallback: number): number {
  const raw = process.env[name];
  const parsed = raw === undefined ? NaN : Number(raw);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

const RATE_LIMIT_MAX = envInt("ADMIN_AUTH_RATE_LIMIT", 5);
const RATE_LIMIT_WINDOW_MS = 60_000; // 1 minute

const BLOCK_THRESHOLD = envInt("ADMIN_AUTH_BLOCK_THRESHOLD", 10);
const BLOCK_WINDOW_MS = envInt("ADMIN_AUTH_BLOCK_WINDOW_MS", 5 * 60_000);
const BLOCK_DURATION_MS = envInt("ADMIN_AUTH_BLOCK_DURATION_MS", 5 * 60_000);

// ── Rate limiter / brute-force tracker state ───────────────────────────────

const rateLimitMap = new Map<string, RateLimitEntry>();
const failureMap = new Map<string, FailureEntry>();

/**
 * Returns true if the request from the given IP is within rate limits,
 * increments the counter, and returns false if the limit has been exceeded.
 */
function checkRateLimit(ip: string): boolean {
  const now = Date.now();
  const entry = rateLimitMap.get(ip);

  if (!entry || now >= entry.resetAt) {
    // First request in this window (or window has expired)
    rateLimitMap.set(ip, { count: 1, resetAt: now + RATE_LIMIT_WINDOW_MS });
    return true;
  }

  entry.count += 1;

  if (entry.count > RATE_LIMIT_MAX) {
    return false;
  }

  return true;
}

/**
 * Returns true if `ip` is currently serving an active brute-force block.
 * Expired blocks are cleared lazily on read.
 */
function isBlocked(ip: string): boolean {
  const entry = failureMap.get(ip);
  if (!entry?.blockedUntil) return false;

  if (Date.now() >= entry.blockedUntil) {
    entry.blockedUntil = null;
    return false;
  }

  return true;
}

/**
 * Records a failed auth attempt for `ip` with a timestamp and, once more
 * than `BLOCK_THRESHOLD` failures have accumulated inside `BLOCK_WINDOW_MS`,
 * blocks the IP for `BLOCK_DURATION_MS` (#318 acceptance criteria).
 */
function recordFailure(ip: string, reason: string): void {
  const now = Date.now();
  const entry = failureMap.get(ip) ?? { timestamps: [], blockedUntil: null };

  // Drop failures outside the rolling block window.
  entry.timestamps = entry.timestamps.filter((t) => now - t < BLOCK_WINDOW_MS);
  entry.timestamps.push(now);

  log("warn", "admin auth failed", {
    ip,
    reason,
    timestamp: new Date(now).toISOString(),
    failuresInWindow: entry.timestamps.length,
  });

  if (entry.timestamps.length > BLOCK_THRESHOLD) {
    entry.blockedUntil = now + BLOCK_DURATION_MS;
    log("warn", "admin auth IP blocked after repeated failures", {
      ip,
      failuresInWindow: entry.timestamps.length,
      threshold: BLOCK_THRESHOLD,
      blockDurationMs: BLOCK_DURATION_MS,
    });
  }

  failureMap.set(ip, entry);
}

// ── Middleware ─────────────────────────────────────────────────────────────

/**
 * Express middleware that:
 * 1. Applies a per-IP rate limit (10 req/min)
 * 2. Validates the JWT from the Authorization: Bearer header
 * 3. Verifies the stellarAddress in the payload matches ADMIN_STELLAR_ADDRESS
 *
 * Calls next() only when all checks pass.
 */
export function adminAuth(req: Request, res: Response, next: NextFunction): void {
  const ip = req.ip ?? req.socket.remoteAddress ?? "unknown";

  // ── Step 0: Brute-force IP block (#318) ────────────────────────────────
  // Checked before the per-minute rate limit so a blocked IP gets a
  // consistent 429 even if its rate-limit window has since reset.
  if (isBlocked(ip)) {
    res.status(429).json({
      error: {
        code: "IP_BLOCKED",
        message: "Too many failed authentication attempts. Try again later.",
        details: [],
      },
    });
    return;
  }

  // ── Step 1: Rate limiting ──────────────────────────────────────────────
  if (!checkRateLimit(ip)) {
    res.status(429).json({
      error: {
        code: "RATE_LIMITED",
        message: "Too many requests",
        details: [],
      },
    });
    return;
  }

  // ── Step 2: Extract token ──────────────────────────────────────────────
  const authHeader = req.headers.authorization;

  if (!authHeader || !authHeader.startsWith("Bearer ")) {
    recordFailure(ip, "missing_authorization_header");
    res.status(401).json({
      error: {
        code: "UNAUTHORIZED",
        message: "Missing Authorization header",
        details: [],
      },
    });
    return;
  }

  const token = authHeader.slice("Bearer ".length);

  // ── Step 3: Verify JWT ─────────────────────────────────────────────────
  const secret = process.env.ADMIN_JWT_SECRET ?? "changeme";
  let payload: AdminTokenPayload;

  try {
    payload = jwt.verify(token, secret) as AdminTokenPayload;
  } catch {
    recordFailure(ip, "invalid_or_expired_token");
    res.status(401).json({
      error: {
        code: "INVALID_TOKEN",
        message: "Invalid or expired token",
        details: [],
      },
    });
    return;
  }

  // ── Step 4: Verify stellarAddress matches admin ────────────────────────
  const adminAddress = process.env.ADMIN_STELLAR_ADDRESS;

  if (!adminAddress || payload.stellarAddress !== adminAddress) {
    recordFailure(ip, "stellar_address_mismatch");
    res.status(403).json({
      error: {
        code: "FORBIDDEN",
        message: "Caller is not the on-chain admin",
        details: [],
      },
    });
    return;
  }

  // ── All checks passed ──────────────────────────────────────────────────
  next();
}
