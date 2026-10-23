# ADR-005 — Multi-Asset Voting Support

**Date:** 2026-09-26  
**Status:** Accepted (v0.2.0+)  
**Author:** VoteChain Contributors  
**Supercedes:** None  
**Related Issues:** #119  

---

## 1. Decision

Extend VoteChain governance to support voting with multiple token assets simultaneously. DAOs can register multiple tokens (e.g., utility tokens + governance NFTs) with per-token weight multipliers, and vote weight is aggregated across all registered tokens.

---

## 2. Context

### 2.1 Current State (v0.1.x)

VoteChain v0.1.1 supports a **single governance token**:
- `initialize()` takes a single `voting_token: Address`
- `cast_vote()` queries the voter's balance in that one token
- Vote weight is purely balance-based (no multipliers)

This model works well for DAOs with a single governance asset, but limits DAOs that have:
- Multiple token classes (e.g., USDC as the utility token + DAO governance token)
- Token hierarchies (e.g., founder tokens worth 2x regular tokens)
- Alternative voting assets (e.g., NFT holdings count as votes)

### 2.2 Problem Statement

Real-world DAOs often issue multiple tokens:

**Example: Compound-like DAO**
- Governance token (COMP): ordinary voting power
- Founder shares (NFT): 10x voting power per share
- Staked LP tokens: 2x voting power per token

Currently, only one token can be used. Founders must either:
1. Vote separately with a manual off-chain aggregation (weak governance).
2. Deploy a separate DAO contract for multi-token support (operational overhead).
3. Accept single-token governance (limits flexibility).

### 2.3 Design Goals

1. **Backward compatible:** v0.1.1 DAOs continue working without changes. Multi-token is opt-in.
2. **Transparent & auditable:** All voting power calculations are visible in proposal events.
3. **Governance-driven token management:** Admin can propose adding/removing tokens via governance (v0.2.1+).
4. **Per-token weight multipliers:** DAOs can configure different tokens to have different voting influence.
5. **Aggregated vote weight:** Final vote weight = sum of (balance_in_token_i × weight_multiplier_i).

---

## 3. Decision

### 3.1 New Data Structures

#### `VotingTokenConfig`

```rust
#[contracttype]
#[derive(Clone, Debug)]
pub struct VotingTokenConfig {
    pub token_address: Address,
    pub weight_multiplier: u64,  // 100 = 1x, 200 = 2x, etc.
}
```

Stored in a list (Vec) in instance storage, keyed by `DataKey::VotingTokens`.

#### Updated `Proposal`

No change to the `Proposal` struct. Vote weight calculation is done at vote time, not stored per-proposal.

### 3.2 Updated Initialization

#### `initialize()` → `initialize_with_tokens()` (Optional, v0.2.0)

New overload for multi-token initialization:

```rust
pub fn initialize_with_tokens(
    env: Env,
    admin: Address,
    voting_tokens: Vec<VotingTokenConfig>,
    min_proposal_balance: i128,
    proposal_cooldown: u64,
    min_duration: u64,
    max_duration: u64,
    restrict_admin_vote: bool,
    timelock_duration: u64,
    max_active_proposals: u64,
) -> Result<(), ContractError>
```

The existing `initialize()` function remains:
- Takes a single `voting_token: Address`
- Wraps it in a `VotingTokenConfig` with `weight_multiplier = 100`
- Calls the internal multi-token initialization logic

**Backward compatibility:** Existing code calling `initialize(voting_token)` continues to work.

### 3.3 Vote Weight Calculation

When `cast_vote()` is called:

```
total_weight = 0

for each VotingTokenConfig (token_address, multiplier):
    balance = query_balance(voter, token_address)
    weight_i = (balance × multiplier) / 100
    total_weight += weight_i

// Ensure no overflow
total_weight = min(total_weight, i128::MAX)
```

**Rounding:** Division by 100 uses truncation (integer division). Weight multipliers should be chosen to avoid rounding artifacts.

**Example:**
- Token A balance: 1,000, multiplier: 100 (1x) → 1,000 votes
- Token B balance: 50, multiplier: 200 (2x) → 100 votes
- Total weight: 1,100 votes

### 3.4 Admin Operations: Token Management

#### `add_voting_token()` (v0.2.0+)

Allows admin to add a new token without re-initialization:

```rust
pub fn add_voting_token(
    env: Env,
    admin: Address,
    token_address: Address,
    weight_multiplier: u64,
) -> Result<(), ContractError>
```

Validation:
- Admin auth required
- Token address not already registered
- Multiplier > 0
- No duplicate registrations

#### `remove_voting_token()` (v0.2.0+)

Allows admin to deregister a token:

```rust
pub fn remove_voting_token(
    env: Env,
    admin: Address,
    token_address: Address,
) -> Result<(), ContractError>
```

Validation:
- Admin auth required
- Token is currently registered
- At least one token remains (cannot remove all tokens)

#### `set_token_weight()` (v0.2.0+)

Allows admin to adjust a token's weight multiplier:

```rust
pub fn set_token_weight(
    env: Env,
    admin: Address,
    token_address: Address,
    new_multiplier: u64,
) -> Result<(), ContractError>
```

### 3.5 Governance-Driven Token Management (v0.2.1+)

Future enhancement: Allow DAOs to propose token additions/removals via governance proposals.

New proposal type:

```rust
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum ProposalType {
    Standard,
    ParameterChange { key: ConfigKey, value: u64 },
    TokenManagement {
        action: TokenAction,
    },
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum TokenAction {
    Add {
        token_address: Address,
        weight_multiplier: u64,
    },
    Remove { token_address: Address },
    UpdateWeight {
        token_address: Address,
        new_multiplier: u64,
    },
}
```

### 3.6 Storage Changes

Add to `DataKey` enum:

```rust
pub enum DataKey {
    // ... existing keys ...
    
    /// List of registered voting tokens with their weight multipliers (instance storage).
    /// Stored as Vec<VotingTokenConfig>.
    VotingTokens,
}
```

The `VotingToken` (singular) key is kept for backward compatibility with v0.1.x DAOs.

### 3.7 Events

New events for multi-token operations:

#### `token_added`
Topics: `("token_added",)`  
Data: `(token_address: Address, weight_multiplier: u64)`

#### `token_removed`
Topics: `("token_removed",)`  
Data: `token_address: Address`

#### `token_weight_updated`
Topics: `("token_weight_upd",)`  
Data: `(token_address: Address, new_multiplier: u64)`

---

## 4. Alternatives Considered

### 4.1 Alternative A: Infinite Tokens (No Limit)

Remove the upper bound on registered tokens.

**Pros:**
- Maximum flexibility
- Future-proof

**Cons:**
- Unbounded gas costs for vote weight calculation
- Storage explosion for DAOs with hundreds of tokens (unlikely but possible)

**Decision:** Defer. Implement a reasonable upper bound (e.g., 16 tokens) in v0.2.0. Lift the limit in v0.3.0 with optimized vote aggregation.

### 4.2 Alternative B: Dynamic Weight Calculation (On-Chain Oracle)

Fetch token weights from an external oracle (e.g., price feed) instead of hard-coding multipliers.

**Pros:**
- Automatically adjusts for market conditions
- No need for proposal votes to rebalance

**Cons:**
- Adds oracle dependency and attack surface
- Complicates voting power calculations
- May be gamed by attacker (flash loan + vote combo)

**Decision:** Defer to v1.0.0. Start with admin-controlled multipliers.

### 4.3 Alternative C: Weighted Voting Pool (Balancer-like)

Treat all tokens as a single liquidity pool with dynamic weights (à la Balancer).

**Pros:**
- Sophisticated voting power model
- Useful for DAOs managing multiple assets

**Cons:**
- High complexity
- Requires Balancer-like math library
- Difficult to audit and test

**Decision:** Defer. Implement simple linear aggregation in v0.2.0.

---

## 5. Rationale

### 5.1 Why Weight Multipliers (not Relative Weights)?

**Weight multiplier** approach:
- `multiplier = 100` → balance is counted as-is (1x)
- `multiplier = 200` → balance counts twice (2x)
- Simple, explicit, easy to calculate

**Relative weight** approach:
- Total weights must sum to 100 or normalized somehow
- Requires re-calculation when tokens are added/removed
- More complex to reason about

We choose multipliers for simplicity and intuitive configuration.

### 5.2 Why Admin-Driven (Not Governance-Driven)?

v0.2.0 uses admin control to add/remove tokens; v0.2.1 will add governance proposals.

**Rationale:**
- Simpler to implement and test
- Admin can respond to emergencies (e.g., compromised token contract)
- Governance token management is a complex proposal type that benefits from separate design

### 5.3 Why Linear Aggregation?

Vote weight = sum of (balance_i × multiplier_i)

**Alternative:** Weighted geometric mean, pool percentages, etc.

**Chosen:** Linear aggregation for transparency and simplicity. DAOs can encode complex policies via multipliers (e.g., set token A weight to 100, token B weight to 200 for 2:1 ratio).

---

## 6. Implementation Plan

### Phase 1: Core Multi-Token Support (v0.2.0) — PROPOSED

- [ ] Add `VotingTokenConfig` type
- [ ] Extend `initialize()` to accept single token; add `initialize_with_tokens()` for multi-token
- [ ] Add `VotingTokens` storage key
- [ ] Update vote weight calculation in `cast_vote()` and `cast_vote_with_delegators()`
- [ ] Implement `add_voting_token()`, `remove_voting_token()`, `set_token_weight()`
- [ ] Add tests for multi-token scenarios
- [ ] Update ADR with implementation notes
- [ ] **Security (issue #309, SEC-014):** implement the per-token balance
      snapshot at proposal-creation time referenced in §7.3 and in
      [SEC-014](security/SEC-014-multi-asset-flash-loan-analysis.md) before
      this phase ships, rather than as a later follow-up — the cross-
      transaction token-recycling risk scales with the number of registered
      tokens and their weight multipliers, so it should be closed up front
      instead of inheriting the single-token "accepted residual risk" status
      (KI-001) for N tokens at once.

### Phase 2: Governance-Driven Token Management (v0.2.1) — PROPOSED

- [ ] Extend `ProposalType` with `TokenManagement` variant
- [ ] Implement `TokenAction` enum
- [ ] Add `create_token_management_proposal()`
- [ ] Update `execute()` to handle token management proposals
- [ ] Add tests for token governance proposals

### Phase 3: Constraints & Limits (v0.3.0) — PROPOSED

- [ ] Enforce maximum registered tokens (e.g., 16 per DAO)
- [ ] Add rate limiting for token additions (e.g., one per timelock period)
- [ ] Implement "freeze" mechanism (admin can freeze voting for specific tokens during emergencies)

---

## 7. Security Considerations

### 7.1 Attack: Token Whitelist Expansion

**Threat:** Admin maliciously registers a compromised token with 100x weight multiplier to manipulate votes.

**Mitigation:**
- All token additions/removals are logged as events and visible on-chain.
- Off-chain monitors can alert if suspicious token is registered.
- Governance-driven token management (v0.2.1) requires a vote.

**Impact:** Medium. Requires governance coordination to respond. Production DAOs should set admin to be a multi-sig or timelock.

### 7.2 Attack: Vote Weight Overflow

**Threat:** Attacker registers many tokens with high multipliers, causing vote weight to overflow i128.

**Mitigation:**
- Vote weight is clamped to `i128::MAX` during aggregation.
- Overflow-safe arithmetic checks prevent panics.
- Tests verify overflow handling.

**Impact:** None—attack is prevented by clamping.

### 7.3 Attack: Flash Loan Across Multiple Tokens

**Threat:** Attacker flash-lends multiple tokens, votes with each, repays all in one transaction.

**Mitigation:**
- Same as single-token flash-loan mitigation (see SEC-011).
- Soroban atomic execution prevents same-transaction exploitation.
- Per-token balance snapshots (future enhancement) would prevent this entirely.

**Impact:** None for same-transaction; residual cross-transaction risk (same as v0.1.x), but amplified relative to v0.1.x because weight is additive across every token in `VotingTokenConfig` — an attacker only needs to source the cheapest/most-liquid registered token, not the DAO's primary governance token specifically. See [SEC-014](security/SEC-014-multi-asset-flash-loan-analysis.md) for the full pre-implementation analysis (issue #309), which rates this **High** pending mitigation and recommends the snapshot option be a blocking requirement for Phase 1 rather than a later enhancement.

### 7.4 Semantic Validation: Token Address Duplication

**Threat:** Admin accidentally registers the same token twice with different multipliers.

**Mitigation:**
- `add_voting_token()` rejects tokens already in the registry.
- Storage is keyed by token address; duplicates overwrite.

**Impact:** None—add operation rejects duplicates.

---

## 8. Testing Strategy

### Unit Tests (v0.2.0)

- ✅ Initialize with multiple tokens
- ✅ Backward compatibility: initialize with single token
- ✅ Vote weight aggregation: multiple tokens with different multipliers
- ✅ Add/remove/update tokens
- ✅ Prevent duplicate token registration
- ✅ Vote weight overflow handling
- ✅ Event emissions for token operations
- ✅ Admin-only token operations

### Integration Tests

- ✅ Full proposal lifecycle with multi-token voting
- ✅ Mixed delegated and direct voting with multiple tokens
- ✅ Parameter changes + token management interaction

---

## 9. Migration & Upgrade Path

### From v0.1.1 to v0.2.0

**Breaking changes:** None. The `initialize()` signature remains compatible.

**New deployments:** Can use `initialize_with_tokens()` for multi-token setups.

**Existing DAOs:** Continue working without modification. They can opt-in to multi-token by:
1. Calling `add_voting_token()` to register additional tokens (requires admin call).
2. Updating governance off-chain to begin proposing with multi-token voting.

---

## 10. Future Enhancements (v0.2.1+)

1. **Governance-driven token management** (ADR-005a)
2. **Per-proposal token subsets** (e.g., "vote only with token A" for specific proposals)
3. **Token veto power** (a specific token can block proposals even if majority votes Yes)
4. **Adaptive weights** (weights change based on governance or oracle)
5. **Token locking** (vote weight locked at proposal creation, like Compound snapshots)

---

## 11. References

- **GitHub Issue:** #119 (Add support for multiple voting token contracts)
- **SEC-011:** Flash-loan investigation (multi-token implications)
- **ADR-004:** Parameter change governance (complements token management)
- **Compound Governor Bravo:** https://github.com/compound-finance/compound-protocol (reference for multi-token voting)
- **Aave Governance:** https://github.com/aave/aave-governance-v2 (reference for complex voting)

---

## 12. Conclusion

Multi-asset voting enables VoteChain to support sophisticated DAOs with multiple governance tokens and assets. By implementing a clear, audit-friendly aggregation model with admin controls and future governance-driven token management, we provide a path for DAOs to evolve their governance structure over time.

The design prioritizes **transparency, simplicity, and security** over feature complexity, with clear upgrade paths for future enhancements.

