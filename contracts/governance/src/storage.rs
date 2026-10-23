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

//! Storage accessors for the governance contract.
//!
//! # Namespacing strategy
//!
//! All storage entries are keyed by variants of [`DataKey`].  Soroban
//! serialises the enum variant discriminant into the XDR key before any
//! payload, so every variant occupies a completely isolated key space.
//! Adding a new data type requires only a new enum variant — there is no
//! risk of collision with existing keys.
//!
//! Storage tiers in use:
//! - **Instance** – singleton config values (`Admin`, `VotingToken`,
//!   `ProposalCount`, `MinProposalBalance`, `ProposalCooldown`, `MinDuration`, `MaxDuration`, `Version`).
//!   Shares the contract instance TTL; cheap to access.
//! - **Persistent** – proposal data and per-voter records (`Proposal`,
//!   `HasVoted`, `VoteRecord`, `VoterSnapshot`, `LastProposal`).
//!   Survives ledger expiry; must be bumped explicitly for long-lived entries.
//!
//! ## Key-space isolation between related variants
//!
//! Several variants share the same payload shape `(u64, Address)` but are
//! distinct enum variants (`HasVoted`, `VoteRecord`, `VoterSnapshot`).
//! Because the discriminant is part of the serialised key, these can never
//! collide even when called with identical arguments.

use crate::types::{ContractError, ContractState, DataKey, PendingMultisigAction, Proposal, ProposalTemplate, VoteRecord};
use soroban_sdk::{Address, Env, Vec};

// =============================================================================
// TTL Constants (Issue #43)
// =============================================================================
//
// Soroban persistent storage entries expire after ~30 days (~2,592,000 ledgers
// at ~1 ledger/second) if not bumped.  Long-lived proposals (up to 30-day voting
// windows) risk having their storage entries expire mid-vote.
//
// These constants match Stellar's ledger TTL settings:
//
//   LEDGER_PER_SECOND ≈ 1  →  1 ledger ≈ 1 second
//   PERSISTENT_ENTRY_EXPIRATION_BUMP ≈ the current Stellar network setting of
//   ~3,110,400 ledgers (≈ 36 days).
//
// We store durations in ledgers (≈ seconds):
//   MIN_TTL  — minimum number of ledgers the entry must still have remaining
//              before we skip the bump.  Set to 30 days so bumps only fire
//              when less than 30 days remain.
//   MAX_TTL  — target TTL to extend to.  Set to 36 days to match the Stellar
//              network persistent-entry bump amount.
//
// References:
//   https://developers.stellar.org/docs/learn/soroban/storage/state-expiration
//   CAP-0046: State Expiration
/// Minimum ledger TTL before a bump is applied (30 days in ledgers ≈ seconds).
pub const TTL_MIN_LEDGERS: u32 = 2_592_000;
/// Target TTL after a bump (36 days in ledgers ≈ seconds).
pub const TTL_MAX_LEDGERS: u32 = 3_110_400;

// =============================================================================
// Storage Strategy
// =============================================================================
//
// Soroban provides three storage tiers. Each key in this contract is assigned
// to the tier that best matches its access pattern and lifetime:
//
// INSTANCE storage  – contract-wide singleton values that share the contract's
//                     TTL. Reads are cheap because the entire instance bucket is
//                     loaded in one host-function call. Used for configuration
//                     that is set once and read on almost every invocation.
//
//   DataKey::Admin              – admin address (set at init, read on admin ops)
//   DataKey::VotingToken        – governance token address (read on every vote)
//   DataKey::ProposalCount      – monotonic proposal ID counter (SEC-007)
//   DataKey::MinProposalBalance – minimum token balance to create a proposal
//   DataKey::ProposalCooldown   – seconds between proposals per address
//   DataKey::ContractState      – lifecycle state (Uninitialized / Ready)
//   DataKey::RestrictAdminVote  – whether admin may vote on own proposals
//   DataKey::Paused             – contract pause flag
//   DataKey::Version            – semver tuple (major, minor, patch)
//
// PERSISTENT storage – per-key TTL, survives ledger expiry independently.
//                      Used for data that must outlive any single ledger and
//                      is keyed by a variable (proposal ID, voter address, etc.).
//
//   DataKey::Proposal(id)                  – full proposal struct
//   DataKey::HasVoted(proposal_id, voter)  – deduplication flag per voter
//   DataKey::VoteRecord(proposal_id, voter)– immutable vote audit record
//   DataKey::VoterSnapshot(proposal_id, voter) – balance snapshot at vote time
//   DataKey::LastProposal(proposer)        – timestamp of proposer's last proposal
//
// TEMPORARY storage  – not used in this contract. Allowances in the token
//                      contract use temporary storage; see token/src/storage.rs.
// =============================================================================

/// Persists a proposal to contract storage, keyed by its ID.
///
/// Also bumps the persistent-storage TTL so the entry survives long voting periods
/// (up to 30 days) without expiring.  See `TTL_MIN_LEDGERS` / `TTL_MAX_LEDGERS`.
pub fn save_proposal(env: &Env, p: &Proposal) {
    let key = DataKey::Proposal(p.id);
    env.storage().persistent().set(&key, p);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_MIN_LEDGERS, TTL_MAX_LEDGERS);
}

/// Loads a proposal from storage by ID.
///
/// Bumps the persistent-storage TTL on every successful read so the entry is not
/// silently evicted while a proposal is still being voted on.
///
/// # Errors
/// - [`ContractError::ProposalNotFound`] if no proposal exists for `id`.
pub fn load_proposal(env: &Env, id: u64) -> Result<Proposal, ContractError> {
    let key = DataKey::Proposal(id);
    let proposal = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(ContractError::ProposalNotFound)?;
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_MIN_LEDGERS, TTL_MAX_LEDGERS);
    Ok(proposal)
}

/// Increments the proposal counter and returns the new ID.
///
/// SEC-007: The counter is the sole source of proposal IDs; no caller-supplied
/// ID is accepted.  The read-increment-write executes atomically within a
/// single Soroban transaction, so concurrent creation attempts (different
/// transactions in the same ledger) each observe a unique counter value.
///
/// # Errors
/// - [`ContractError::ProposalCountOverflow`] if the counter would exceed `u64::MAX`.
pub fn next_id(env: &Env) -> Result<u64, ContractError> {
    let current: u64 = env
        .storage()
        .instance()
        .get(&DataKey::ProposalCount)
        .unwrap_or(0);
    let n = current
        .checked_add(1)
        .ok_or(ContractError::ProposalCountOverflow)?;
    env.storage().instance().set(&DataKey::ProposalCount, &n);
    Ok(n)
}

/// Stores the admin address in instance storage.
pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

/// Returns `true` if the contract has been initialised (admin key exists).
pub fn is_initialized(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Admin)
}

/// Stores the contract lifecycle state in instance storage.
pub fn set_contract_state(env: &Env, state: &ContractState) {
    env.storage().instance().set(&DataKey::ContractState, state);
}

/// Returns the contract lifecycle state.
///
/// Defaults to [`ContractState::Uninitialized`] if the key has never been written
/// (i.e. before the very first `initialize` call).
pub fn get_contract_state(env: &Env) -> ContractState {
    env.storage()
        .instance()
        .get(&DataKey::ContractState)
        .unwrap_or(ContractState::Uninitialized)
}

/// Returns the stored admin address.
///
/// # Errors
/// - [`ContractError::AdminNotSet`] if the contract has not been initialised.
pub fn get_admin(env: &Env) -> Result<Address, ContractError> {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(ContractError::AdminNotSet)
}

/// Stores the governance token address in instance storage.
pub fn set_voting_token(env: &Env, token: &Address) {
    env.storage().instance().set(&DataKey::VotingToken, token);
}

/// Returns the stored governance token address.
///
/// # Errors
/// - [`ContractError::VotingTokenNotSet`] if the contract has not been initialised.
pub fn get_voting_token(env: &Env) -> Result<Address, ContractError> {
    env.storage()
        .instance()
        .get(&DataKey::VotingToken)
        .ok_or(ContractError::VotingTokenNotSet)
}

/// Records that `voter` has voted on `proposal_id`.
pub fn mark_voted(env: &Env, proposal_id: u64, voter: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::HasVoted(proposal_id, voter.clone()), &true);
}

/// Returns `true` if `voter` has already voted on `proposal_id`.
pub fn has_voted(env: &Env, proposal_id: u64, voter: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::HasVoted(proposal_id, voter.clone()))
        .unwrap_or(false)
}

/// Records that `proposal_id`'s voting period has been extended once via `extend_voting`.
pub fn mark_extended(env: &Env, proposal_id: u64) {
    env.storage()
        .persistent()
        .set(&DataKey::ProposalExtended(proposal_id), &true);
}

/// Returns `true` if `proposal_id`'s voting period has already been extended once.
pub fn has_extended(env: &Env, proposal_id: u64) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::ProposalExtended(proposal_id))
        .unwrap_or(false)
}

/// Stores the vote record for `voter` on `proposal_id`.
pub fn save_vote_record(env: &Env, proposal_id: u64, voter: &Address, record: &VoteRecord) {
    env.storage()
        .persistent()
        .set(&DataKey::VoteRecord(proposal_id, voter.clone()), record);
}

/// Returns the vote record for `voter` on `proposal_id`, or `None` if not voted.
pub fn get_vote_record(env: &Env, proposal_id: u64, voter: &Address) -> Option<VoteRecord> {
    env.storage()
        .persistent()
        .get(&DataKey::VoteRecord(proposal_id, voter.clone()))
}

pub fn set_min_proposal_balance(env: &Env, v: i128) {
    env.storage()
        .instance()
        .set(&DataKey::MinProposalBalance, &v);
}

pub fn get_min_proposal_balance(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::MinProposalBalance)
        .unwrap_or(0)
}

pub fn set_proposal_cooldown(env: &Env, v: u64) {
    env.storage().instance().set(&DataKey::ProposalCooldown, &v);
}

pub fn get_proposal_cooldown(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&DataKey::ProposalCooldown)
        .unwrap_or(0)
}

pub fn set_last_proposal(env: &Env, proposer: &Address, ts: u64) {
    env.storage()
        .persistent()
        .set(&DataKey::LastProposal(proposer.clone()), &ts);
}

pub fn get_last_proposal(env: &Env, proposer: &Address) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::LastProposal(proposer.clone()))
        .unwrap_or(0)
}

/// Records the voter's token balance snapshot for a given proposal.
/// Called once per voter per proposal at the time of casting their vote.
pub fn save_voter_snapshot(env: &Env, proposal_id: u64, voter: &Address, weight: i128) {
    env.storage()
        .persistent()
        .set(&DataKey::VoterSnapshot(proposal_id, voter.clone()), &weight);
}

/// Returns the stored vote-weight snapshot for a voter on a proposal.
/// Returns `None` if no snapshot has been recorded yet.
pub fn get_voter_snapshot(env: &Env, proposal_id: u64, voter: &Address) -> Option<i128> {
    env.storage()
        .persistent()
        .get(&DataKey::VoterSnapshot(proposal_id, voter.clone()))
}
/// Stores the contract version as a `(major, minor, patch)` tuple.
pub fn set_version(env: &Env, version: (u32, u32, u32)) {
    env.storage().instance().set(&DataKey::Version, &version);
}

/// Returns the stored contract version as a `(major, minor, patch)` tuple.
pub fn get_version(env: &Env) -> (u32, u32, u32) {
    env.storage()
        .instance()
        .get(&DataKey::Version)
        .unwrap_or((0, 0, 0))
}

/// Stores whether admin is restricted from voting on their own proposals.
pub fn set_restrict_admin_vote(env: &Env, v: bool) {
    env.storage()
        .instance()
        .set(&DataKey::RestrictAdminVote, &v);
}

/// Returns whether admin vote restriction is enabled. Defaults to `false`.
pub fn get_restrict_admin_vote(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::RestrictAdminVote)
        .unwrap_or(false)
}

/// Stores the mandatory delay (seconds) a passed proposal must wait before it can be executed.
pub fn set_timelock_duration(env: &Env, v: u64) {
    env.storage().instance().set(&DataKey::TimelockDuration, &v);
}

/// Returns the configured timelock duration in seconds. Defaults to 0 (no delay).
pub fn get_timelock_duration(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&DataKey::TimelockDuration)
        .unwrap_or(0)
}

/// Sets the contract paused state.
pub fn set_paused(env: &Env, paused: bool) {
    env.storage().instance().set(&DataKey::Paused, &paused);
}

/// Returns `true` if the contract is currently paused.
pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false)
}

pub fn set_min_duration(env: &Env, v: u64) {
    env.storage().instance().set(&DataKey::MinDuration, &v);
}

pub fn get_min_duration(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&DataKey::MinDuration)
        .unwrap_or(60)
}

pub fn set_max_duration(env: &Env, v: u64) {
    env.storage().instance().set(&DataKey::MaxDuration, &v);
}

pub fn get_max_duration(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&DataKey::MaxDuration)
        .unwrap_or(2_592_000)
}

/// Stores the pending admin address nominated for rotation.
pub fn set_pending_admin(env: &Env, addr: &Address) {
    env.storage().instance().set(&DataKey::PendingAdmin, addr);
}

/// Returns the pending admin address, if any.
pub fn get_pending_admin(env: &Env) -> Option<Address> {
    env.storage().instance().get(&DataKey::PendingAdmin)
}

/// Clears the pending admin nomination.
pub fn clear_pending_admin(env: &Env) {
    env.storage().instance().remove(&DataKey::PendingAdmin);
    env.storage()
        .instance()
        .remove(&DataKey::AdminTransferExpiry);
}

/// Stores the expiry timestamp for the pending admin nomination.
pub fn set_admin_transfer_expiry(env: &Env, ts: u64) {
    env.storage()
        .instance()
        .set(&DataKey::AdminTransferExpiry, &ts);
}

/// Returns the expiry timestamp for the pending admin nomination.
pub fn get_admin_transfer_expiry(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&DataKey::AdminTransferExpiry)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Delegation storage
// ---------------------------------------------------------------------------

/// Records that `delegator` has delegated their voting power to `delegate`.
///
/// Stored in persistent storage so that delegations survive ledger expiry
/// without needing to be re-submitted on every proposal.
pub fn set_delegation(env: &Env, delegator: &Address, delegate: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::Delegation(delegator.clone()), delegate);
}

/// Returns the address to which `delegator` has delegated, or `None` if the
/// delegator has not delegated their voting power.
pub fn get_delegation(env: &Env, delegator: &Address) -> Option<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::Delegation(delegator.clone()))
}

/// Removes the delegation record for `delegator`.
///
/// After this call `get_delegation(delegator)` returns `None` and the
/// delegator may vote directly again.
pub fn clear_delegation(env: &Env, delegator: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::Delegation(delegator.clone()));
}

/// Stores the previous WASM hash for rollback purposes.
pub fn set_previous_wasm_hash(env: &Env, hash: &soroban_sdk::BytesN<32>) {
    env.storage()
        .instance()
        .set(&DataKey::PreviousWasmHash, hash);
}

/// Returns the previous WASM hash, if any.
pub fn get_previous_wasm_hash(env: &Env) -> Option<soroban_sdk::BytesN<32>> {
    env.storage().instance().get(&DataKey::PreviousWasmHash)
}

// ---------------------------------------------------------------------------
// Active proposal cap and counter (#34 — TooManyActiveProposals)
// ---------------------------------------------------------------------------

const DEFAULT_MAX_ACTIVE_PROPOSALS: u64 = 50;

/// Stores the global cap on simultaneously active proposals.
pub fn set_max_active_proposals(env: &Env, cap: u64) {
    env.storage()
        .instance()
        .set(&DataKey::MaxActiveProposals, &cap);
}

/// Returns the global cap on simultaneously active proposals.
/// Defaults to 50 if not explicitly configured.
pub fn get_max_active_proposals(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&DataKey::MaxActiveProposals)
        .unwrap_or(DEFAULT_MAX_ACTIVE_PROPOSALS)
}

/// Returns the current count of active proposals.
pub fn count_active_proposals(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&DataKey::ActiveProposalsCount)
        .unwrap_or(0)
}

/// Increments the active-proposal counter by one.
pub fn increment_active_proposals(env: &Env) {
    let current = count_active_proposals(env);
    env.storage()
        .instance()
        .set(&DataKey::ActiveProposalsCount, &(current + 1));
}

/// Decrements the active-proposal counter by one (saturating at zero).
pub fn decrement_active_proposals(env: &Env) {
    let current = count_active_proposals(env);
    env.storage()
        .instance()
        .set(&DataKey::ActiveProposalsCount, &current.saturating_sub(1));
}

// ---------------------------------------------------------------------------
// Proposal template storage (issue #324)
// ---------------------------------------------------------------------------

/// Maximum byte length of a template ID string.
pub const MAX_TEMPLATE_ID_LEN: u32 = 64;

/// Stores a proposal template in persistent storage, keyed by its template ID.
///
/// Also bumps the persistent-storage TTL so the entry survives long-term
/// without expiring.
pub fn save_template(env: &Env, template: &ProposalTemplate) {
    let key = DataKey::ProposalTemplate(template.template_id.clone());
    env.storage().persistent().set(&key, template);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_MIN_LEDGERS, TTL_MAX_LEDGERS);
}

/// Loads a proposal template from storage by its string ID.
///
/// Bumps the persistent-storage TTL on every successful read.
///
/// # Errors
/// - [`ContractError::TemplateNotFound`] if no template exists for `template_id`.
pub fn load_template(env: &Env, template_id: &String) -> Result<ProposalTemplate, ContractError> {
    let key = DataKey::ProposalTemplate(template_id.clone());
    let template = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(ContractError::TemplateNotFound)?;
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_MIN_LEDGERS, TTL_MAX_LEDGERS);
    Ok(template)
}

/// Returns `true` if a template with the given ID already exists.
pub fn has_template(env: &Env, template_id: &String) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::ProposalTemplate(template_id.clone()))
}

/// Removes a proposal template from storage. Returns `true` if a template
/// was deleted, `false` if no template existed for the given ID.
pub fn remove_template(env: &Env, template_id: &String) -> bool {
    let key = DataKey::ProposalTemplate(template_id.clone());
    let existed = env.storage().persistent().has(&key);
    if existed {
        env.storage().persistent().remove(&key);
    }
    existed
}

/// Stores the minimum quorum floor (in basis points) in instance storage.
/// `10` BPS = 0.1 % of total supply.
pub fn set_min_quorum_bps(env: &Env, bps: u32) {
    env.storage().instance().set(&DataKey::MinQuorumBps, &bps);
}

/// Returns the stored minimum quorum BPS floor.
/// Defaults to `10` (0.1 % of supply) if never set.
pub fn get_min_quorum_bps(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::MinQuorumBps)
        .unwrap_or(10)
}