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
 * In-memory API key store for the verified-DAO tier.
 *
 * Keys are 32-byte random values encoded as hex. Only an Argon2id hash of
 * the raw key is stored server-side (see SEC-015); the raw key is returned
 * exactly once at creation and again at rotation, then discarded. The first
 * 8 characters of the raw key are kept in plaintext as `keyPrefix` purely
 * so operators can identify a key in listings — this is not sensitive
 * material and does not weaken the hash.
 *
 * This implementation is intentionally in-memory for the initial release.
 * Replacing the backing store with PostgreSQL or Redis is a single-file
 * concern — all callers import from this module only.
 *
 * Tiers:
 *   - "anon"         — unauthenticated IP-based limits (100 read / 10 write / min)
 *   - "verified-dao" — authenticated DAO key limits    (1000 read / 100 write / min)
 */

import crypto from "crypto";
import argon2 from "argon2";

// ── Tier definitions ───────────────────────────────────────────────────────

export type ApiKeyTier = "anon" | "verified-dao";

export interface TierLimits {
  readPerMinute: number;
  writePerMinute: number;
}

export const TIER_LIMITS: Record<ApiKeyTier, TierLimits> = {
  anon: { readPerMinute: 100, writePerMinute: 10 },
  "verified-dao": { readPerMinute: 1000, writePerMinute: 100 },
};

// ── Data model ─────────────────────────────────────────────────────────────

/** Number of leading characters of the raw key kept in plaintext for display. */
const KEY_PREFIX_LENGTH = 8;

export interface ApiKeyRecord {
  /** Opaque ID used to reference the key in management endpoints. */
  id: string;
  /** DAO identifier supplied by the admin at creation time. */
  daoId: string;
  /** Argon2id hash of the raw key — never stored in plaintext. */
  argonHash: string;
  /** First 8 characters of the raw key, kept in plaintext for display only. */
  keyPrefix: string;
  /** Tier granted to this key. */
  tier: ApiKeyTier;
  /** ISO-8601 timestamp of key creation. */
  createdAt: string;
  /** ISO-8601 timestamp of last rotation, or null if never rotated. */
  rotatedAt: string | null;
  /** ISO-8601 timestamp of revocation, or null if still active. */
  revokedAt: string | null;
  /**
   * Whether abuse detection has suspended this key.
   * Suspended keys are rejected with HTTP 403.
   */
  suspended: boolean;
  /** ISO-8601 timestamp when the key was suspended, or null. */
  suspendedAt: string | null;
}

/** Sanitised view returned to callers — omits the stored hash. */
export type ApiKeyView = Omit<ApiKeyRecord, "argonHash">;

// ── Key generation / hashing helpers ────────────────────────────────────────

/** Generates a cryptographically-random 32-byte API key as a lowercase hex string. */
export function generateRawKey(): string {
  return crypto.randomBytes(32).toString("hex");
}

/**
 * Hashes a raw key with Argon2id. The returned string embeds the salt and
 * tuning parameters, so no separate salt storage is required.
 */
export async function hashKey(rawKey: string): Promise<string> {
  return argon2.hash(rawKey, { type: argon2.argon2id });
}

/**
 * Constant-effort verification of a raw key against a stored Argon2id hash.
 * Delegates to argon2.verify, which performs the comparison itself; callers
 * iterating over multiple candidate records should await every candidate
 * (never short-circuit on the first match) to avoid leaking timing info
 * about which candidate matched.
 */
export async function verifyKey(argonHash: string, rawKey: string): Promise<boolean> {
  try {
    return await argon2.verify(argonHash, rawKey);
  } catch {
    // Malformed hash (shouldn't happen for records we created) — treat as no match.
    return false;
  }
}

// ── Store ──────────────────────────────────────────────────────────────────

/** Primary store: keyId → record */
const store = new Map<string, ApiKeyRecord>();

/**
 * Secondary index: keyPrefix → set of keyIds whose raw key starts with that
 * prefix. Argon2id hashes are salted, so (unlike the old SHA-256 scheme)
 * there is no way to index directly by hash. The prefix narrows an incoming
 * request to a small candidate set; each candidate is then verified with
 * argon2.verify. Prefix collisions are rare (1 in 16^8) and only cost an
 * extra verify call, never a false authentication.
 */
const prefixIndex = new Map<string, Set<string>>();

function addToPrefixIndex(prefix: string, keyId: string): void {
  let set = prefixIndex.get(prefix);
  if (!set) {
    set = new Set();
    prefixIndex.set(prefix, set);
  }
  set.add(keyId);
}

function removeFromPrefixIndex(prefix: string, keyId: string): void {
  const set = prefixIndex.get(prefix);
  if (!set) return;
  set.delete(keyId);
  if (set.size === 0) prefixIndex.delete(prefix);
}

// ── CRUD operations ────────────────────────────────────────────────────────

export interface CreateKeyResult {
  record: ApiKeyView;
  /** Raw key — shown exactly once. Store it securely; it cannot be retrieved again. */
  rawKey: string;
}

/**
 * Creates a new verified-DAO API key for the given DAO.
 *
 * @param daoId     Human-readable identifier for the DAO (e.g. "astro-dao").
 * @param tier      Tier to grant; defaults to "verified-dao".
 * @returns         The sanitised record and the raw key (shown once).
 */
export async function createApiKey(
  daoId: string,
  tier: ApiKeyTier = "verified-dao"
): Promise<CreateKeyResult> {
  const rawKey = generateRawKey();
  const argonHash = await hashKey(rawKey);
  const keyPrefix = rawKey.slice(0, KEY_PREFIX_LENGTH);
  const id = crypto.randomUUID();
  const now = new Date().toISOString();

  const record: ApiKeyRecord = {
    id,
    daoId,
    argonHash,
    keyPrefix,
    tier,
    createdAt: now,
    rotatedAt: null,
    revokedAt: null,
    suspended: false,
    suspendedAt: null,
  };

  store.set(id, record);
  addToPrefixIndex(keyPrefix, id);

  const { argonHash: _omit, ...view } = record;
  return { record: view, rawKey };
}

export interface RotateKeyResult {
  record: ApiKeyView;
  rawKey: string;
}

/**
 * Rotates an existing key: invalidates the old key hash and issues a new one.
 * The old raw key immediately stops working.
 *
 * @throws {Error} if the key does not exist or is already revoked.
 */
export async function rotateApiKey(keyId: string): Promise<RotateKeyResult> {
  const record = store.get(keyId);
  if (!record) throw new Error(`API key not found: ${keyId}`);
  if (record.revokedAt) throw new Error(`API key already revoked: ${keyId}`);

  // Remove old prefix entry from index
  removeFromPrefixIndex(record.keyPrefix, keyId);

  // Generate new key material
  const rawKey = generateRawKey();
  record.argonHash = await hashKey(rawKey);
  record.keyPrefix = rawKey.slice(0, KEY_PREFIX_LENGTH);
  record.rotatedAt = new Date().toISOString();
  // Unsuspend on rotation — gives the DAO a clean slate
  record.suspended = false;
  record.suspendedAt = null;

  addToPrefixIndex(record.keyPrefix, keyId);

  const { argonHash: _omit, ...view } = record;
  return { record: view, rawKey };
}

/**
 * Revokes a key. Revoked keys are permanently rejected with HTTP 401.
 *
 * @throws {Error} if the key does not exist.
 */
export function revokeApiKey(keyId: string): ApiKeyView {
  const record = store.get(keyId);
  if (!record) throw new Error(`API key not found: ${keyId}`);

  removeFromPrefixIndex(record.keyPrefix, keyId);
  record.revokedAt = new Date().toISOString();

  const { argonHash: _omit, ...view } = record;
  return view;
}

/**
 * Suspends a key due to abuse detection.
 * Unlike revocation, suspension can be lifted by rotating the key.
 */
export function suspendApiKey(keyId: string): void {
  const record = store.get(keyId);
  if (!record) return; // no-op if already deleted
  record.suspended = true;
  record.suspendedAt = new Date().toISOString();
}

/**
 * Returns the sanitised view of a key record by ID, or null if not found.
 */
export function getApiKeyById(keyId: string): ApiKeyView | null {
  const record = store.get(keyId);
  if (!record) return null;
  const { argonHash: _omit, ...view } = record;
  return view;
}

/**
 * Looks up the full record by an incoming raw key, verifying it against the
 * stored Argon2id hash.
 *
 * Narrows candidates by key prefix (cheap, not security-sensitive) and then
 * verifies every candidate with argon2.verify rather than stopping at the
 * first match, so lookup time doesn't vary with which (if any) candidate
 * matched.
 *
 * @returns The internal record (including argonHash) or null if not found.
 */
export async function lookupByRawKey(rawKey: string): Promise<ApiKeyRecord | null> {
  if (rawKey.length < KEY_PREFIX_LENGTH) return null;
  const prefix = rawKey.slice(0, KEY_PREFIX_LENGTH);
  const candidateIds = prefixIndex.get(prefix);
  if (!candidateIds || candidateIds.size === 0) return null;

  const candidates = Array.from(candidateIds)
    .map((id) => store.get(id))
    .filter((r): r is ApiKeyRecord => r !== undefined);

  const results = await Promise.all(
    candidates.map(async (record) => ({
      record,
      match: await verifyKey(record.argonHash, rawKey),
    }))
  );

  return results.find((r) => r.match)?.record ?? null;
}

/**
 * Returns all key records as sanitised views. Admin-only.
 */
export function listApiKeys(): ApiKeyView[] {
  return Array.from(store.values()).map(({ argonHash: _omit, ...view }) => view);
}

// ── Test helpers (not exported in production use) ──────────────────────────

/**
 * Clears the store completely. For use in tests only.
 * @internal
 */
export function _clearStoreForTesting(): void {
  store.clear();
  prefixIndex.clear();
}
