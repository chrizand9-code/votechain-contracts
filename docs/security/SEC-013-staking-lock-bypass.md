# SEC-013: Staking Lock-Bypass Attack Analysis

| Field | Value |
|---|---|
| Record type | Security analysis |
| Date | 2026-09-29 |
| Status | Resolved — mitigations implemented in `contracts/token/src/staking.rs` and `contracts/token/src/lib.rs` (`transfer`/`transfer_from`), with regression tests in `contracts/token/src/test.rs` (see issue #316) |
| Severity | Not recorded |
| Affected components | `contracts/token/src/staking.rs`, `contracts/token/src/lib.rs` — `stake`, `unstake`, `transfer`, `transfer_from` |
| Reporter / reviewer | Not recorded |

Related issue: [#116 — Add governance token staking for time-weighted voting power](https://github.com/veracindarella/votechain-contracts/issues/116). Format: [SEC template](SEC-TEMPLATE.md).

---

## 1. Background

ADR-013 introduces a ve-token staking model in which governance token holders
lock their tokens for up to 365 days in exchange for a voting-power multiplier
of up to 4×.  The security property that must hold is:

> **Lock Invariant:** A staker's locked tokens MUST NOT be transferable to
> another address while `env.ledger().timestamp() < stake_record.locked_until`.

Violating this invariant allows an attacker to:

1. Lock tokens to acquire a boosted voting power.
2. Transfer (or effectively transfer) the same tokens to a second address.
3. Have the second address also stake, doubling (or more) the effective
   voting power derived from a single underlying token balance.

This document identifies the attack surfaces and the required mitigations.

---

## 2. Attack Surface 1 — Lock Bypass via Allowance Trick

### 2.1 Attack Description

The ERC-20 / SEP-41 allowance mechanism (`approve` + `transfer_from`) creates
a potential bypass:

1. Alice holds 1,000 tokens and stakes all of them for 365 days → 4× voting
   power (4,000 effective votes).
2. **Before staking**, Alice calls `approve(alice, bob, 1_000)`.  The allowance
   is stored in temporary storage and is NOT invalidated by the subsequent
   `stake` call (if `stake` does not clear existing allowances).
3. Bob calls `transfer_from(bob, alice, carol, 1_000)`.
4. If `transfer_from` does not check Alice's staking record, the 1,000 tokens
   are transferred to Carol despite being locked.
5. Carol stakes the same 1,000 tokens → another 4,000 effective votes.
6. Alice still has her `StakeRecord` pointing to 1,000 locked tokens; the
   `staked_voting_power` function reads it and still returns 4,000.

**Net result:** 8,000 effective votes from 1,000 underlying tokens.

### 2.2 Root Cause

`transfer_from` checks only:

- Spender allowance ≥ amount ✓
- Owner balance ≥ amount ✓ (if staked tokens are still counted in total balance)

It does **not** check whether the tokens are subject to a lock.

### 2.3 Required Mitigation

Both `transfer` and `transfer_from` must enforce the lock before moving tokens:

```rust
// In transfer() and transfer_from(), before deducting from balance:
if let Some(record) = load_stake_record(&env, &from) {
    let transferable = balance - record.amount;
    if amount > transferable {
        return Err(ContractError::TokensLocked);
    }
}
```

Where `balance` is the total balance and `record.amount` is the staked
(frozen) portion.  Only `balance - record.amount` (the unstaked portion) is
transferable while the lock is active.

Additionally, `approve` should be blocked (or at minimum warn) when the
requested amount exceeds the transferable balance, to prevent pre-approving
locked tokens:

```rust
// Optional hardening in approve():
let transferable = total_balance - staked_amount;
if amount > transferable {
    return Err(ContractError::AllowanceExceedsTransferableBalance);
}
```

---

## 3. Attack Surface 2 — Flash Loan Stake Attack

### 3.1 Attack Description

A flash loan allows borrowing a large token balance within a single transaction
(atomic borrow + use + repay).  In a naive staking implementation:

1. Attacker takes a flash loan of N tokens.
2. Calls `stake(attacker, N, 365 * 86_400)` → receives 4× multiplier.
3. Calls `cast_vote` in the governance contract → votes with 4N effective power.
4. Calls `unstake(attacker)` (if lock bypass is also possible) and repays loan.

**Note:** Flash loans require a lending contract that exists on the same chain.
On Stellar/Soroban, true atomic flash loans (borrow + use + repay in one
transaction) require a compatible lending protocol to be deployed.  As of the
time of writing, no such protocol exists on Soroban mainnet.  However, this
should be treated as a forward-looking risk.

### 3.2 Why Standard Lock Prevents This

If `unstake` correctly enforces `locked_until`, the attacker cannot repay the
flash loan within the same transaction because:

- `unstake` will revert with `StakeLocked` if called before `locked_until`.
- A 365-day lock means the attacker's tokens (and the borrowed tokens) are
  frozen for 365 days.
- The attacker cannot repay the flash loan → the entire transaction reverts.

**The lock itself is the primary flash loan mitigation**, provided the lock
cannot be bypassed (see §2).

### 3.3 Residual Risk and Mitigation

| Scenario | Risk level | Mitigation |
|----------|-----------|-----------|
| Flash loan with 0-day lock | **Low** — 1× multiplier, no boost | No additional mitigation needed |
| Flash loan with long lock | **None** — attacker's tokens are frozen | Lock enforcement in `unstake` |
| Flash loan combined with lock bypass (§2) | **High** | Fix lock bypass first (§2.3) |
| Flash loan with social attack (attacker accepts permanent lock cost) | **Medium** | Quorum thresholds; admin oversight |

For a 365-day lock and 4× multiplier, the economic cost of a flash loan attack
(bearing a year of illiquidity for the loaned amount) makes the attack
economically irrational for all but the most adversarial actors.

---

## 4. Attack Surface 3 — Stale StakeRecord After Full Transfer

### 4.1 Attack Description

If a user transfers their **entire** token balance by exploiting the lock bypass
(§2), the `StakeRecord` in persistent storage is not automatically deleted.
Subsequent calls to `staked_voting_power` will:

1. Load the (now stale) `StakeRecord`.
2. Compute `record.amount * multiplier / MULTIPLIER_PRECISION`.
3. Return a positive voting power even though the address holds zero tokens.

This creates phantom voting power uncoupled from any token holding.

### 4.2 Mitigation

This is a downstream consequence of the lock bypass (§2); fixing the root cause
(§2.3) prevents the stale record from arising.  As defence in depth:

```rust
// In staked_voting_power():
let raw_balance = load_balance(&env, &staker);
if raw_balance == 0 {
    return 0; // No balance → no voting power regardless of stake record
}
// ... existing multiplier logic
```

Additionally, `staked_voting_power` should cap the staked amount at the
current balance to handle edge cases:

```rust
let effective_staked = record.amount.min(raw_balance);
let staked_power = effective_staked * multiplier / MULTIPLIER_PRECISION;
let unstaked_power = raw_balance - effective_staked; // at 1×
staked_power + unstaked_power
```

---

## 5. Attack Surface 4 — Lock Duration Manipulation

### 5.1 Attack Description

An attacker who can supply an arbitrarily large `lock_duration_seconds` to
`stake()` could:

- Overflow `locked_until` (if `env.ledger().timestamp() + lock_duration_seconds`
  overflows `u64`), resulting in `locked_until = 0` and an immediately
  expirable lock.
- Receive the maximum 4× multiplier with an effectively zero lock period.

### 5.2 Mitigation

Validate the lock duration before computing `locked_until`:

```rust
const MAX_LOCK_SECONDS: u64 = MAX_LOCK_DAYS * SECONDS_PER_DAY; // 31_536_000

if lock_duration_seconds > MAX_LOCK_SECONDS {
    return Err(ContractError::InvalidLockDuration);
}

let locked_until = env
    .ledger()
    .timestamp()
    .checked_add(lock_duration_seconds)
    .ok_or(ContractError::ArithmeticOverflow)?;
```

Use `checked_add` on `locked_until` to prevent silent overflow.

---

## 6. Summary of Required Checks

The following table lists every security check that MUST be present in the
implementation before this feature is deployed:

| Location | Check | Prevents |
|----------|-------|---------|
| `transfer` | `amount <= balance - staked_amount` while lock active | Lock bypass via direct transfer |
| `transfer_from` | `amount <= balance - staked_amount` while lock active | Lock bypass via allowance trick |
| `unstake` | `env.ledger().timestamp() >= locked_until` | Premature unstaking |
| `stake` | `lock_duration_seconds <= MAX_LOCK_SECONDS` | Duration overflow |
| `stake` | `checked_add` for `locked_until` computation | Integer overflow in timestamp |
| `stake` | `amount <= balance - already_staked` | Over-staking |
| `staked_voting_power` | Cap staked amount at current balance | Phantom voting power |
| `staked_voting_power` | Return 0 if raw balance == 0 | Stale record exploit |

---

## 7. Verdict

| Attack | Exploitable pre-mitigation? | Mitigation |
|--------|----------------------------|------------|
| Lock bypass via allowance trick (§2) | **Yes** — if `transfer_from` ignores staking records | Check `amount <= transferable_balance` in `transfer`/`transfer_from` |
| Flash loan stake attack (§3) | **No** — lock itself prevents repayment | Lock enforcement in `unstake` |
| Stale StakeRecord phantom votes (§4) | **Yes** — downstream of bypass in §2 | Fix §2 + defensive cap in `staked_voting_power` |
| Lock duration overflow (§5) | **Yes** — if no validation | `checked_add`, cap at `MAX_LOCK_SECONDS` |

**Overall:** The lock bypass via allowance and the duration overflow are the
two highest-priority issues.  Both have clear, low-complexity mitigations.
The flash loan attack is self-mitigated by the lock design.

### 7.1 Implementation Note (issue #316)

All checks in §6 are now implemented:

- `stake`/`unstake`/`staked_voting_power` in `staking.rs` are fully
  implemented (no longer `todo!()` stubs), including `checked_add` for
  `locked_until`, a `MAX_LOCK_DAYS` duration cap, and the balance-capped,
  zero-balance-safe `staked_voting_power` computation from §4.
- `TokenContract::transfer` and `TokenContract::transfer_from` in `lib.rs`
  both call `staking::transferable_balance` before debiting `from`'s
  balance, returning `ContractError::TokensLocked` if `amount` exceeds the
  unstaked portion — this closes the allowance-trick bypass in §2 for both
  direct transfers and `transfer_from`.
- Regression tests reproducing the attack from §2.1 (approve-before-stake,
  then `transfer_from` after the stake is active) and covering the other
  checks in §6 live in `contracts/token/src/test.rs`.

---

## 8. References

- [ADR-013](../adr/ADR-013-ve-token-staking.md) — ve-token design decision
- [SEC-010](SEC-010-reentrancy-cast-vote.md) — Reentrancy analysis for `cast_vote`
- [SEC-011](SEC-011-flash-loan-investigation.md) — Flash loan investigation
- [contracts/token/src/staking.rs](../../contracts/token/src/staking.rs) — Staking scaffold
- Curve Finance veCRV audit: https://curve.fi/audits
- Soroban checked arithmetic: https://docs.rs/soroban-sdk/
