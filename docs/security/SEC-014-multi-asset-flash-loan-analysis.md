# SEC-014: Flash-loan attack surface for multi-asset voting (ADR-005)

| Field | Value |
|---|---|
| Record type | Pre-implementation threat analysis |
| Date | 2026-10-21 |
| Status | Informational — no code changed yet; ADR-005 is not implemented |
| Severity | High (pre-mitigation), if ADR-005 ships without a lock/snapshot control |
| Affected components | `docs/ADR-005-multi-asset-voting.md` (design), future `cast_vote` multi-token weight aggregation, `VotingTokenConfig` |
| Reporter / reviewer | Security review for issue #309 |

Format: [SEC template](SEC-TEMPLATE.md). Builds on the single-token analysis in
[SEC-011](SEC-011-flash-loan-investigation.md) and the threat actors defined in
[threat-model.md](threat-model.md) (T1, T5).

## Summary

[SEC-011](SEC-011-flash-loan-investigation.md) concluded that same-transaction
flash-loan attacks are **not exploitable** against the current single-token
governance contract, because Soroban's atomic, non-reentrant execution model
means a voter's balance cannot be inflated and repaid within the same
transaction that calls `cast_vote`.

ADR-005 ("Multi-Asset Voting Support") proposes aggregating vote weight across
**multiple** registered tokens:
`total_weight = Σ (balance_in_token_i × weight_multiplier_i)`.

This document re-runs the flash-loan threat analysis against that proposed
design. The single-transaction atomicity argument from SEC-011 still holds for
*each individual token read* — reading `token::balance()` for token A and then
token B within the same call to `cast_vote` is still synchronous and
non-reentrant, so no token's balance can be flash-borrowed and repaid between
reads. **However, multi-asset voting introduces a new, qualitatively different
risk that SEC-011 did not need to consider: cross-token substitution within a
single voting window, using one borrowed/liquid token among several as the
amplification vector.** Because ADR-005 sums weight across N tokens rather than
reading one, the attack surface scales with the number of registered tokens
and with how liquid/borrowable each one is — the "weakest link" token in the
`VotingTokenConfig` list determines the realistic worst case, not the
strongest one.

## Impact

- **Affected asset:** proposal outcomes (vote tallies), and by extension any
  value gated by a passed proposal (fund releases, parameter changes, admin
  rotation via governance-approved upgrades).
- **Affected users:** all token holders of the DAO; a successful attack lets a
  minority stakeholder manufacture artificial majority support.
- **Prerequisites:** at least one of the registered voting tokens in
  `VotingTokenConfig` must be available via a flash loan or similarly
  deep, instantaneously borrowable liquidity venue (e.g. a Soroban
  liquidity-pool/AMM flash-swap, or — once bridged — an EVM-side flash loan
  against a wrapped representation of the token, if the token is bridgeable).
  A token that is *illiquid or non-transferable in the same block* (e.g. an
  NFT with a transfer-cooldown) is not a viable vector by itself but still
  needs to be ruled out per-token when new tokens are registered.
- **Realistic consequence, assuming no mitigation ships with ADR-005:**
  - Within-block attack: still blocked by Soroban atomicity for any *single*
    `cast_vote` call, exactly as in SEC-011 — a loan taken out and repaid
    inside the same transaction that calls `cast_vote` cannot inflate that
    call's weight, because the balance read happens once, synchronously,
    inside that transaction, and the repayment (if it happens later in the
    *same* transaction) cannot retroactively change an already-computed sum.
  - **Cross-transaction / within-voting-window attack (the real new risk):**
    an attacker can borrow token A, cast a vote (locking in a high aggregated
    weight for that one vote), repay the loan in a later transaction in the
    same block or a subsequent block, and the recorded vote weight is
    unaffected because — like the single-token case — weight is read live
    *only at vote time*, not continuously. This is the same "live balance,
    not snapshot" problem SEC-011 identified for KI-001, but **multi-asset
    voting multiplies the number of exploitable tokens**: instead of one
    token to acquire, an attacker only needs to flash-borrow *the single
    cheapest-to-borrow token among N*, since any one token's balance
    contributes additively to total weight. This lowers the cost of an
    attack that is already rated "accepted residual risk" for the
    single-token case (KI-001), because the attacker now has an N-way choice
    of which token to target and can pick the most liquid/cheapest one.
  - Per-token weight multipliers (`weight_multiplier`) make this worse, not
    better: a token configured with a high multiplier (e.g. "founder shares:
    10x") is a much higher-value target for borrowing/acquiring temporarily
    than the base governance token, even if its absolute liquidity is lower.

## Analysis

### 1. Same-transaction case — still not exploitable (unchanged from SEC-011)

For a single call to `cast_vote` (or its future multi-token equivalent), all
`token::balance()` reads across the registered tokens happen synchronously,
in sequence, within one Soroban transaction, before the vote record and
`has_voted` guard are written. Soroban's non-reentrant execution model (see
SEC-011 §3.1–3.3) means no external call back into the governance contract,
nor a balance mutation on any of the N tokens, can occur between the first and
last balance read in that call. The multi-token case does not weaken this
property — it just means there are N reads chained together instead of 1,
all still inside the same atomic transaction.

### 2. Cross-transaction case — amplified by token count and multipliers (new)

This is the SEC-011 KI-001 "token recycling" risk, generalized:

```
Transaction T1 (within the proposal's voting window):
  1. Attacker flash-borrows (or otherwise temporarily acquires) a large
     balance of Token_k, the cheapest/most-liquid token in the DAO's
     VotingTokenConfig list with the highest weight_multiplier-to-cost ratio.
  2. Attacker calls cast_vote(attacker, proposal_id, Vote::Yes)
     - Weight = Σ_i balance_i(attacker) × multiplier_i
     - Token_k's borrowed balance dominates the sum.
     - has_voted[proposal_id][attacker] = true (vote locked in)
  3. Attacker repays/returns Token_k (same block or a later one).

Result: the attacker's recorded vote weight reflects a balance they held only
transiently, and — because has_voted prevents a second vote from the same
address — the inflated weight is permanent for that proposal's tally, same as
single-token KI-001, but reachable via whichever token in the list is
cheapest to source rather than requiring the DAO's primary governance token
specifically.
```

Constraints that limit (but do not eliminate) this, inherited from SEC-011:
on-chain visibility of all transfers/borrows, the `has_voted` one-vote-per-
address guard, and the bounded voting window. New constraint introduced by
ADR-005: the attack is bounded by `Σ weight_multiplier_i × borrowable_supply_i`
across registered tokens, so a DAO with only illiquid/low-multiplier
secondary tokens narrows the realistic attack surface versus one that
registers a highly liquid token with a large multiplier.

### 3. Risk rating (using the threat-model.md actor framework)

Mapped to threat actor **T1 — Malicious voter** (threat-model.md §4): "cast
more votes than their token balance entitles them to." Multi-asset voting
does not introduce a new actor, but it increases T1's attack surface because
weight is additive across tokens the attacker does not need to hold
long-term.

| Dimension | Single-token (SEC-011 / KI-001) | Multi-asset (ADR-005, this doc) |
|---|---|---|
| Same-tx flash loan | Not exploitable | Not exploitable (unchanged) |
| Cross-tx recycling | Possible, Medium residual risk | Possible, **High** — attack surface scales with number/liquidity of registered tokens and their multipliers |
| Mitigation cost | SC-020 snapshot (planned, not shipped) | Same snapshot mechanism, but must cover **all** registered tokens, not just one |

**Rating: High**, pending mitigation, specifically because ADR-005 is not yet
implemented — this is the point in the design process where the mitigation
should be designed in rather than retrofitted (unlike the single-token case,
where the contract already shipped before KI-001/SEC-011 were written).

## Remediation or mitigation

Per the ADR-005 acceptance criteria, two mitigation options were evaluated:

1. **Balance lock period** — require tokens counted toward vote weight to
   have been held (untransferred) for a minimum duration before the vote is
   cast. Pro: no new storage schema, works with existing `token::balance`
   reads plus a transfer-timestamp check. Con: requires the token contract to
   expose last-transfer-time per holder, which SEP-41 tokens do not
   standardly provide; would require a VoteChain-specific token extension or
   an off-chain attestation, adding trust-boundary surface.
2. **Snapshot at proposal creation** — record each registered token's
   `weight_multiplier`-adjusted balance for a voter at proposal-creation time
   (or at first-vote time, as a per-proposal snapshot), rather than reading
   live balances at vote time. Pro: directly closes the cross-transaction
   recycling/flash-source vector for every registered token at once, and is
   the same mechanism already recommended for the single-token case (SC-020
   in SEC-011 §6.2) — implementing it for multi-asset voting at the same time
   avoids building two separate snapshot systems. Con: higher storage cost
   (one snapshot entry per voter per token per proposal, versus a live read),
   and requires deciding the snapshot moment (proposal creation vs. first
   vote) to avoid grief vectors where early voters get a stale weight.

**Recommendation:** adopt the **snapshot option**, and treat it as the same
underlying mechanism as SC-020 (tracked in SEC-011), extended to iterate over
`VotingTokenConfig` instead of a single token address. This should be a
blocking design requirement for ADR-005's implementation tasks, not a
follow-up — unlike the single-token contract, which shipped before this
residual risk was discovered, ADR-005 has not shipped yet, so there is no
backward-compatibility cost to requiring it up front. The balance-lock-period
option is not recommended as the primary control because it depends on
token-contract capabilities VoteChain does not control for externally
registered tokens.

## Verification

No code exists yet to verify. ADR-005 is "Accepted (v0.2.0+)" and the
`VotingTokenConfig` struct it describes already exists as a type definition
(`contracts/governance/src/types.rs:251`), but the contract does not yet use
it: `lib.rs` only exposes the single-token `initialize()`, and there is no
`initialize_with_tokens()` entry point or multi-token weight-aggregation logic
wired into `create_proposal`/`cast_vote`. This document is pending
verification as follows:

- **Pending:** when `initialize_with_tokens()` and multi-token weight
  aggregation are implemented, add contract tests mirroring
  `test_flash_loan_attack_across_transactions_demonstration` (SEC-011) but
  parameterized over ≥2 registered tokens with different
  `weight_multiplier` values, confirming (a) same-transaction borrow+vote+
  repay across any subset of the registered tokens cannot inflate a single
  `cast_vote` call's recorded weight, and (b) if/when the snapshot mitigation
  lands, that a token balance change after the snapshot point does not alter
  an already-cast vote's weight.
- **Pending:** update `known-issues.md` KI-001 to cross-reference this
  document once ADR-005 implementation begins, analogous to how SEC-011
  documents the single-token case today.

## References

- [SEC-011 — Flash-Loan Attack Investigation & KI-001 Analysis](SEC-011-flash-loan-investigation.md)
- [ADR-005 — Multi-Asset Voting Support](../ADR-005-multi-asset-voting.md)
- [threat-model.md](threat-model.md) — actor T1 (malicious voter), T5 (compromised token contract)
- [known-issues.md](known-issues.md) — KI-001 (vote weight recycling, no snapshot mechanism)
- Issue #309
