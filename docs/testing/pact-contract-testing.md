# Consumer-Driven Contract Testing (SDK ↔ Backend)

**Issue:** #308 — "Add API contract tests between backend and SDK"
**Status:** Phase 1 (proposal-read endpoints) implemented; CI gate and
vote-endpoint coverage are follow-up work, documented below.

## Why

The SDK (`sdk/`) and the backend (`backend/`) are separately versioned
packages in the same repo. Nothing currently prevents the backend from
changing a response shape (field renamed, type changed, pagination format
altered) without the SDK's expectations being updated in lockstep, other than
manual review. [Pact](https://docs.pact.io/) consumer-driven contract tests
close that gap: the consumer (SDK) defines the interactions it expects, the
tests run against a Pact mock server to produce a "pact file," and that pact
file can later be replayed against the real provider (backend) to verify it
still honours the contract.

## What is covered today

`sdk/src/__contract__/proposals.pact.spec.ts` defines Pact interactions for
the two HTTP endpoints the backend actually exposes for proposals, per
`backend/src/routes/proposals.ts`:

- `GET /proposals` — list endpoint, verified against the `{ data: [...],
  pagination: {...} }` shape returned by the route's cursor-pagination logic.
- `GET /proposals/:id` — single-proposal lookup, verified against the `{ id
  }` shape the stub handler currently returns.

## What is explicitly *not* covered yet, and why

The issue's acceptance criteria mention "proposal and vote endpoints."
Auditing `backend/src/routes/` (see the directory listing below) found **no
HTTP vote endpoint**:

```
backend/src/routes/
  analytics.ts  apiKeys.ts  csv.ts  health.ts  keys.ts
  leaderboard.ts  proposals.ts  simulate.ts  webhooks.ts
```

Voting in VoteChain happens by calling the Soroban governance contract
directly — the SDK's `GovernanceClient` (`sdk/src/client.ts`) builds and signs
a transaction against the contract, it is not proxied through a backend REST
endpoint. There is therefore no backend HTTP contract to write for "vote" yet.
If/when a backend vote-submission or vote-status endpoint is added, add a
corresponding Pact interaction here following the same pattern as the
proposal endpoints above — that is a natural first extension of this file
rather than a new mechanism.

## Running the contract tests

The contract tests depend on `@pact-foundation/pact`, declared in
`sdk/package.json` `devDependencies` but intentionally **not** bundled into
the default `npm test` run:

- `sdk/vitest.config.ts` only includes `src/**/*.test.ts`.
- The contract spec is named `proposals.pact.spec.ts` (not `*.test.ts`) and
  has its own config, `sdk/vitest.contract.config.ts`, which includes
  `src/**/*.pact.spec.ts`.
- Run it explicitly with `npm --prefix sdk run test:contract`.

This keeps `npm test` dependency-free of Pact for contributors who only touch
unrelated SDK code, while still making the contract tests a first-class,
runnable suite.

## CI wiring (follow-up, not yet done)

The acceptance criteria ask for:

> Contract test failures block merge to main

This is **not yet wired into CI** as part of this change. Adding a new
required status check (a `pact-contract` job in a workflow, analogous to how
`enforce-severity-gate` blocks on CodeQL findings — see
`docs/security/sast-scanning.md`) changes branch-protection requirements for
the whole repository and should be reviewed/approved by a maintainer with
admin access rather than added unilaterally in a contributor PR. The
recommended follow-up:

1. Add a `pact.yml` workflow (or a job in the SDK's existing CI workflow)
   that runs `npm --prefix sdk ci && npm --prefix sdk run test:contract`.
2. Publish the resulting pact file as a build artifact (or to a Pact Broker,
   if one is stood up) so the backend side can eventually verify against it
   in its own CI job (provider verification), not just generate it.
3. Mark the new job as a required status check once it has run clean on a
   few PRs, mirroring how `enforce-severity-gate` was adopted for CodeQL.

## References

- `sdk/src/__contract__/proposals.pact.spec.ts` — the contract test itself
- `backend/src/routes/proposals.ts` — the provider endpoints under contract
- `sdk/src/client.ts` — why there is no HTTP vote endpoint to contract-test
- [Pact docs](https://docs.pact.io/)
- Issue #308
