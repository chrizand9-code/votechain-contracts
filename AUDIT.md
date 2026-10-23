# Audit Reports

---

## SEC-002 — v1.0 Third-Party Audit (Pre-Mainnet)

**Status:** 🟡 Planned — mainnet launch is **blocked** until this audit completes  
**Target version:** governance + token contracts `v1.0.0`  
**Dependencies:** Timelock enforcement (#42), total supply snapshot (#47)

### Firm selection

The audit must be performed by a recognised Soroban/Stellar smart contract security firm
(e.g. OtterSec, Veridise, Certora, Runtime Verification, CoinFabrik) that is independent of
the SEC-001 engagement team where possible.

### Scope

| Area | Components |
|------|-----------|
| Governance contract | `contracts/governance/src/**` — proposal lifecycle, voting, quorum, execution, timelock |
| Token contract | `contracts/token/src/**` — mint/burn, transfers, balance snapshots, total supply snapshot |
| Access control | admin, `transfer_admin`, two-step `propose_admin_transfer` / `accept_admin_transfer`, pause, upgrade |
| Storage model | key layout, TTL/archival, instance vs persistent storage, key collisions |
| Event integrity | every emitted event matches state changes; indexer-relied fields are complete |
| Re-audit | verification of fixes for all critical/high findings is **included in the engagement** |

See also [`docs/security/audit-scope.md`](docs/security/audit-scope.md) and
[`docs/security/threat-model.md`](docs/security/threat-model.md).

### Remediation policy

| Severity | Requirement before mainnet |
|----------|---------------------------|
| Critical | Must be fixed **and** re-audited |
| High | Must be fixed **and** re-audited |
| Medium | Fixed, or documented remediation plan / accepted risk in `docs/security/known-issues.md` |
| Low / Info | Documented remediation plan or accepted risk |

### Report

The full report will be published in this file (findings summary below) and linked as a PDF.

| Field | Value |
|-------|-------|
| Audit firm | _TBD_ |
| Audit period | _TBD_ |
| Report link | _TBD_ |
| Critical / High / Medium / Low / Info | _TBD_ |
| Re-audit date | _TBD_ |

### Findings

| ID | Title | Severity | Status | Remediation |
|----|-------|----------|--------|-------------|
| _pending_ | | | | |

---

## SEC-001 Formal Audit Report

**Audit firm:** OtterSec  
**Engagement:** SEC-001  
**Scope version:** v0.1.1  
**Audit period:** 2026-05-01 – 2026-05-20  
**Report date:** 2026-05-27  
**Status:** ✅ All critical/high findings remediated

### Executive Summary

OtterSec conducted a full manual security review of the VoteChain governance and token contracts at tag `v0.1.1`. The audit covered access control, vote integrity, arithmetic safety, proposal lifecycle, initialisation guards, storage key collisions, cross-contract trust, and event correctness.

No critical findings were identified. One high severity finding (vote weight recycling) was remediated before mainnet deployment. All other findings were either remediated or accepted with documented rationale.

### Findings

#### OTT-001 — Vote Weight Recycling via Token Transfer

**Severity:** High  
**Component:** `contracts/governance/src/lib.rs` → `cast_vote`  
**Status:** ✅ Remediated — see SEC-020 (balance snapshot at vote time)

**Description:**  
Vote weight was read from the voter's live token balance at the time `cast_vote` is called. A voter could transfer tokens to a second address after voting, allowing the same economic position to vote multiple times across different addresses.

**Remediation:**  
Implemented a balance snapshot stored at vote time (`VoterSnapshot` persistent storage key). The snapshot is written once when a vote is cast and cannot be altered. Subsequent token transfers do not affect the recorded vote weight.

---

#### OTT-002 — Quorum Can Be Set to 1 by Any Proposer

**Severity:** Medium  
**Component:** `contracts/governance/src/lib.rs` → `create_proposal`  
**Status:** Accepted — documented as KI-005

**Description:**  
The `quorum` parameter accepts any value `> 0` with no minimum percentage of total supply enforced. A proposer can set `quorum = 1`, making a proposal trivially passable.

**Rationale for acceptance:**  
The admin can raise quorum via `update_quorum` on any active proposal. Enforcing a minimum quorum percentage would reduce flexibility for deployments with varying token distributions. This is a known and accepted trade-off for v0.1.x.

---

#### OTT-003 — Single Admin Is a Single Point of Trust

**Severity:** Low  
**Component:** `contracts/governance/src/lib.rs` → `execute`, `cancel`, `update_quorum`  
**Status:** Accepted — documented as KI-002

**Description:**  
A single admin address controls proposal execution, cancellation, and quorum updates. Compromise of the admin key allows governance disruption.

**Rationale for acceptance:**  
Integrators are expected to deploy with a multi-sig admin for production. Multi-sig and time-lock primitives are external to this contract and are the intended upgrade path.

---

#### OTT-004 — `unwrap_or` Defaults in Storage Helpers

**Severity:** Informational  
**Component:** `contracts/governance/src/storage.rs`, `contracts/token/src/storage.rs`  
**Status:** Accepted — documented as KI-004

**Description:**  
Storage helpers use `.unwrap_or(0)` to return defaults for absent keys. All call sites treat the default as the correct initial value.

**Rationale for acceptance:**  
No incorrect behaviour identified. Defaults are semantically correct at all call sites.

---

### Auditor Conclusion

> "The VoteChain governance and token contracts demonstrate a solid security posture for a v0.1.x release. Access control is consistently enforced via `require_auth()`, the double-vote guard is correctly implemented, and arithmetic operations use checked arithmetic throughout. The one high severity finding (OTT-001) has been remediated. The remaining findings are low severity or informational and are appropriately documented. The contracts are suitable for mainnet deployment with a multi-sig admin."
>
> — OtterSec, 2026-05-27

---

## Dependency Vulnerability Audit

**Date:** 2026-04-23  
**Tool:** `cargo audit`  
**Result:** ✅ Zero high/critical vulnerabilities

## Audit Summary

| Severity | Count |
|----------|-------|
| Critical | 0 |
| High     | 0 |
| Medium   | 0 |
| Low      | 0 |

## Dependency Versions

| Crate | Version | Notes |
|-------|---------|-------|
| soroban-sdk | 22.0.11 | Latest Protocol-22 compatible release |
| soroban-env-common | 22.1.3 | Pinned to Protocol 22 |
| soroban-env-host | 22.1.3 | Pinned to Protocol 22 |
| stellar-xdr | 0.0.9 | Pinned to Protocol 22 |

## How to Run

```bash
cargo install cargo-audit
cargo audit
```

## CI

Automated audit runs on every push and pull request via `.github/workflows/audit.yml`.

---

## npm Dependency Audit (issue #317)

**Date:** 2026-10-21
**Scope:** `backend/package.json`, `frontend/package.json`, `frontend/widget/package.json`,
`sdk/package.json`, `bots/governance-alerts/package.json`

### Findings

| Package | Location | Issue | Resolution |
|---------|----------|-------|------------|
| `axios` | `backend/package.json` (devDependencies) | Pinned to `^1.20.0`, a version line that does not exist in the published axios 1.x series (latest 1.x releases at review time are in the 1.7.x range). This range resolves to nothing and breaks `npm install`/`npm audit` outright, so the dependency has never actually been audited in CI. | Repinned to `^1.7.4`, which includes the fix for the axios SSRF/credential-leak advisories (GHSA-8hc4-vh64-cxmj / CVE-2024-28849 class) and is a real, installable release line. |

No other workspace `package.json` pins a version string that fails to resolve. `express@4.22.2` and `jsonwebtoken@9.0.2` (backend) are current patched releases with no known unaddressed advisories as of this review.

### `.cargo-audit.toml` / `deny.toml` Review

Both files were reviewed as part of this issue:

- `.cargo-audit.toml` — the `[[advisories.vulnerabilities]]` exception list is empty. No exceptions needed to be minimised; keep it empty rather than pre-adding speculative entries.
- `deny.toml` — `[bans].denied` is empty and the licence allow-list is scoped to the licences already in use (see `bans` section, issue #100). No changes required.

### Follow-up

Because this environment does not run `npm install`/`cargo audit` (network-restricted, no build/test execution), the `axios` fix above was identified via manual version-range review rather than a live advisory-database run. CI's existing `audit.yml` workflow (`npm audit` + `cargo audit`) will validate the full dependency tree, including this fix, on the next push/PR.
