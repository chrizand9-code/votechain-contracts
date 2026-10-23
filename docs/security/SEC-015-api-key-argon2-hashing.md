# SEC-015: API key hashing at rest (Argon2id)

| Field | Value |
|---|---|
| Record type | Finding and fix |
| Date | 2026-10-21 |
| Status | Fixed |
| Severity | Critical |
| Affected components | `backend/src/middleware/apiKeyStore.ts`, `backend/src/routes/keys.ts` |
| Reporter / reviewer | Issue #311 |

## Summary

`apiKeyStore.ts` stored API keys as a plain SHA-256 digest with no salt or
work factor. SHA-256 is fast to compute, so a database breach (or a copy of
the in-memory heap) let an attacker brute-force every key offline at GPU
speed. Keys are now hashed with Argon2id, a memory-hard KDF, before storage.

## Impact

Any read access to the key store (DB dump, memory dump, backup leak) would
have let an attacker recover every verified-DAO API key by brute force,
since SHA-256 has no built-in cost factor to slow that down. A recovered key
grants the verified-DAO rate-limit tier and whatever the owning DAO's key is
scoped to in `docs/api-keys.md`.

## Analysis

- `hashKey()` previously called `crypto.createHash("sha256")` with no salt.
- The reverse-lookup index (`hashIndex: Map<keyHash, keyId>`) relied on the
  hash being deterministic, which let an attacker precompute a rainbow table
  once and reuse it against every record in the store.
- Only `apiKeyStore.ts` (the in-memory store wired into `routes/keys.ts`) was
  in scope for this fix, since it is the module the issue named.
- Note: `backend/src/services/apiKeyService.ts` (the Redis-backed store used
  by the currently-mounted `routes/apiKeys.ts` / `apiKeyAuth.ts` path) also
  hashes with SHA-256. That is a defensible design for *that* module
  specifically because the material it hashes is a full 256-bit
  cryptographically random token (`vc_<64 hex chars>`), not a low-entropy
  secret — the same threat model GitHub and Stripe use SHA-256 for on their
  PATs/API keys. It is flagged here as a follow-up to consider, but changing
  it is out of scope for this fix and would need its own issue, since the
  lookup index is a live Redis key (`apikey:lookup:<sha256>`) and switching
  to a salted KDF there requires either a scan-and-verify lookup path or a
  schema migration — the same architectural change made below for
  `apiKeyStore.ts`.

## Remediation

- `hashKey()` now calls `argon2.hash(rawKey, { type: argon2.argon2id })`.
- `ApiKeyRecord.keyHash` was replaced with `argonHash` (the full Argon2id
  output, which embeds its own salt and parameters) plus a new `keyPrefix`
  field: the first 8 characters of the raw key, stored in plaintext purely
  for display in key listings (plaintext because it is not sensitive on its
  own — it is 1/8th of the key material, nowhere near enough entropy to
  authenticate).
- Lookup can no longer index directly by hash (Argon2id hashes are salted),
  so `lookupByRawKey()` now narrows candidates by `keyPrefix` via a
  `prefixIndex: Map<prefix, Set<keyId>>` and verifies every candidate in
  that (usually single-element) set with `argon2.verify()`, awaiting all
  candidates via `Promise.all` rather than short-circuiting on the first
  match, to avoid a timing side-channel across candidates.
- `createApiKey()` and `rotateApiKey()` are now `async` to accommodate the
  Argon2id hash computation; `backend/src/routes/keys.ts` route handlers
  were updated to `await` them.
- Added the `argon2` package to `backend/package.json` dependencies.

A dedicated migration script for existing plaintext/SHA-256 keys was not
added: the module is in-memory only (restarts clear it, and the module's own
doc comment already states this is intentional for the initial release), so
there is no persisted data to migrate. If/when this store is backed by a
real database, the migration would re-hash on next successful
authentication (since the raw key is available at that moment) and mark
unmigrated records for forced rotation — that approach should be documented
in a follow-up SEC note at that time.

## Verification

Pending: `backend/tests/apiKeyAuth.test.ts` was updated to `await` the now-
async `createApiKey()` calls it exercises. Running `vitest` was out of scope
for this change (the task instructions this fix was produced under
disallow running build/test/install commands); a maintainer should run
`npm test` in `backend/` before merging to confirm the suite still passes
with the new async API and the `argon2` dependency installed.

## References

- Issue #311
- `backend/src/middleware/apiKeyStore.ts`
- `backend/src/routes/keys.ts`
- `backend/tests/apiKeyAuth.test.ts`
