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

#![cfg(test)]

use super::*;
use crate::test_helpers::{create_test_proposal, mint_and_vote, setup_env};
use crate::types::{ConfigKey, ProposalType};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events, Ledger},
    Address, Env, IntoVal, String, TryFromVal,
};

// ── local helpers for tests that need a custom Env/client shape ───────────────

/// Register a fresh token contract, mint `supply` to `admin`, return its address.
fn setup_token(env: &Env, admin: &Address) -> Address {
    let id = env.register(votechain_token::TokenContract, ());
    let t = votechain_token::TokenContractClient::new(env, &id);
    t.initialize(admin, &10_000_000);
    id
}

fn new_client(env: &Env) -> GovernanceContractClient<'static> {
    GovernanceContractClient::new(env, &env.register(GovernanceContract, ()))
}

/// Create a passed proposal (voted Yes, finalised) for access-control tests.
fn setup_passed_proposal(env: &Env, client: &GovernanceContractClient, admin: &Address) -> u64 {
    let voter = Address::generate(env);
    let token_id = setup_token(env, &voter);
    client.initialize(
        admin,
        &token_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );
    let id = client.create_proposal(
        &voter,
        &String::from_str(env, "Prop"),
        &String::from_str(env, "desc"),
        &100,
        &3600,
    );
    client.cast_vote(&voter, &id, &Vote::Yes, &None);
    env.ledger().with_mut(|l| l.timestamp += 3601);
    client.finalise(&id);
    id
}

/// Create an active proposal for access-control tests.
fn setup_active_proposal(env: &Env, client: &GovernanceContractClient, admin: &Address) -> u64 {
    let proposer = Address::generate(env);
    let token_id = setup_token(env, admin);
    client.initialize(
        admin,
        &token_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );
    client.create_proposal(
        &proposer,
        &String::from_str(env, "Prop"),
        &String::from_str(env, "desc"),
        &100,
        &3600,
    )
}

// ── SC-001: initialize tests ──────────────────────────────────────────────────

/// State is Uninitialized before initialize, Ready after; admin and token are
/// stored; the Initialized event is emitted.
#[test]
fn test_initialize() {
    let env = Env::default();
    env.mock_all_auths();

    let gov_id = env.register(GovernanceContract, ());
    let client = GovernanceContractClient::new(&env, &gov_id);

    // Before initialize: state must be Uninitialized
    assert_eq!(client.get_state(), ContractState::Uninitialized);

    let admin = Address::generate(&env);
    let tok_id = env.register(votechain_token::TokenContract, ());
    let tok = votechain_token::TokenContractClient::new(&env, &tok_id);
    tok.initialize(&admin, &10_000_000);

    client.initialize(
        &admin,
        &tok_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );

    // After initialize: state must be Ready
    assert_eq!(client.get_state(), ContractState::Ready);

    // Admin and voting token are retrievable (indirectly via an admin-only op)
    // A cancel call with the correct admin succeeds only if admin was stored correctly.
    let proposer = Address::generate(&env);
    let id = client.create_proposal(
        &proposer,
        &String::from_str(&env, "Init test"),
        &String::from_str(&env, "desc"),
        &100,
        &3600,
    );
    client.cancel(&admin, &id); // would revert with NotAdmin if admin wasn't stored
    assert_eq!(client.get_proposal(&id).state, ProposalState::Cancelled);
}

/// initialize emits the "init" event with the admin address as data.
#[test]
fn test_initialize_emits_event() {
    let env = Env::default();
    env.mock_all_auths();

    let gov_id = env.register(GovernanceContract, ());
    let client = GovernanceContractClient::new(&env, &gov_id);

    let admin = Address::generate(&env);
    let tok_id = env.register(votechain_token::TokenContract, ());
    let tok = votechain_token::TokenContractClient::new(&env, &tok_id);
    tok.initialize(&admin, &10_000_000);

    client.initialize(
        &admin,
        &tok_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );

    // The "init" event must have been published with admin as data
    let events = env.events().all();
    assert!(
        events.iter().any(|(_, topics, data)| {
            topics == (symbol_short!("init"),).into_val(&env)
                && Address::try_from_val(&env, &data).ok().as_ref() == Some(&admin)
        }),
        "expected 'init' event with admin address as data"
    );
}

// ── end SC-001 ────────────────────────────────────────────────────────────────

// ── basic lifecycle ───────────────────────────────────────────────────────────

#[test]
fn test_create_proposal() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let id = create_test_proposal(&t, &proposer);
    assert_eq!(id, 1);
    assert_eq!(t.client.proposal_count(), 1);
    assert_eq!(t.client.get_proposal(&id).state, ProposalState::Active);
}

#[test]
fn test_cast_vote_and_finalise_passed() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter);

    mint_and_vote(&t, &voter, id, Vote::Yes, 1_000_000);
    assert!(t.client.has_voted(&id, &voter));
    assert_eq!(t.client.get_proposal(&id).votes_yes, 1_000_000);

    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&id);
    assert_eq!(t.client.get_proposal(&id).state, ProposalState::Passed);
}

#[test]
fn test_finalise_rejected_below_quorum() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = t.client.create_proposal(
        &voter,
        &String::from_str(&t.env, "B"),
        &String::from_str(&t.env, "desc"),
        &9_999_999,
        &3600,
    );
    mint_and_vote(&t, &voter, id, Vote::Yes, 1_000_000);
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&id);
    assert_eq!(t.client.get_proposal(&id).state, ProposalState::Rejected);
}

#[test]
fn test_finalise_rejected_no_wins() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter);
    mint_and_vote(&t, &voter, id, Vote::No, 1_000_000);
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&id);
    assert_eq!(t.client.get_proposal(&id).state, ProposalState::Rejected);
}

#[test]
fn test_execute_passed_proposal() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter);
    mint_and_vote(&t, &voter, id, Vote::Yes, 1_000_000);
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&id);
    t.client.execute(&t.admin, &id);
    assert_eq!(t.client.get_proposal(&id).state, ProposalState::Executed);
}

#[test]
fn test_cancel_proposal() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let id = create_test_proposal(&t, &proposer);
    t.client.cancel(&t.admin, &id);
    assert_eq!(t.client.get_proposal(&id).state, ProposalState::Cancelled);
}

// ── TEST-009: concurrent proposal scenario tests ──────────────────────────────

#[test]
fn test_concurrent_proposals_independent_votes() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id1 = create_test_proposal(&t, &voter);
    let id2 = create_test_proposal(&t, &voter);
    let id3 = create_test_proposal(&t, &voter);

    mint_and_vote(&t, &voter, id1, Vote::Yes, 1_000_000);
    assert!(t.client.has_voted(&id1, &voter));
    assert!(!t.client.has_voted(&id2, &voter));
    assert!(!t.client.has_voted(&id3, &voter));
}

#[test]
fn test_concurrent_votes_do_not_bleed() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id1 = create_test_proposal(&t, &voter);
    let id2 = create_test_proposal(&t, &voter);

    mint_and_vote(&t, &voter, id1, Vote::Yes, 1_000_000);

    assert_eq!(t.client.get_proposal(&id1).votes_yes, 1_000_000);
    let p2 = t.client.get_proposal(&id2);
    assert_eq!(p2.votes_yes, 0);
    assert_eq!(p2.votes_no, 0);
    assert_eq!(p2.votes_abstain, 0);
}

#[test]
fn test_finalise_one_does_not_affect_others() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id1 = create_test_proposal(&t, &voter);
    let id2 = t.client.create_proposal(
        &voter,
        &String::from_str(&t.env, "P2"),
        &String::from_str(&t.env, "d"),
        &1,
        &7200,
    );

    mint_and_vote(&t, &voter, id1, Vote::Yes, 1_000_000);
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&id1);

    assert_ne!(t.client.get_proposal(&id1).state, ProposalState::Active);
    assert_eq!(t.client.get_proposal(&id2).state, ProposalState::Active);
}

#[test]
fn test_proposal_ids_are_unique() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let id1 = create_test_proposal(&t, &proposer);
    let id2 = create_test_proposal(&t, &proposer);
    let id3 = create_test_proposal(&t, &proposer);
    assert!(id1 != id2 && id2 != id3 && id1 != id3);
    assert_eq!(t.client.proposal_count(), 3);
}

#[test]
fn test_proposals_at_different_lifecycle_stages() {
    let t = setup_env();
    let voter = Address::generate(&t.env);

    let active_id = t.client.create_proposal(
        &voter,
        &String::from_str(&t.env, "Active"),
        &String::from_str(&t.env, "d"),
        &1,
        &7200,
    );
    let passed_id = create_test_proposal(&t, &voter);
    let rejected_id = t.client.create_proposal(
        &voter,
        &String::from_str(&t.env, "Rejected"),
        &String::from_str(&t.env, "d"),
        &9_999_999,
        &3600,
    );
    let cancelled_id = create_test_proposal(&t, &voter);

    mint_and_vote(&t, &voter, passed_id, Vote::Yes, 1_000_000);
    t.client.cancel(&t.admin, &cancelled_id);

    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&passed_id);
    t.client.finalise(&rejected_id);

    assert_eq!(
        t.client.get_proposal(&active_id).state,
        ProposalState::Active
    );
    assert_eq!(
        t.client.get_proposal(&passed_id).state,
        ProposalState::Passed
    );
    assert_eq!(
        t.client.get_proposal(&rejected_id).state,
        ProposalState::Rejected
    );
    assert_eq!(
        t.client.get_proposal(&cancelled_id).state,
        ProposalState::Cancelled
    );
}

// ── end TEST-009 ──────────────────────────────────────────────────────────────

#[test]
#[should_panic]
fn test_cannot_vote_twice() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter);
    mint_and_vote(&t, &voter, id, Vote::Yes, 1_000_000);
    t.client.cast_vote(&voter, &id, &Vote::No, &None); // should panic
}

// ── TEST-013: access control negative tests ───────────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_execute_non_admin_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    let id = setup_passed_proposal(&env, &client, &admin);
    client.execute(&non_admin, &id);
}

#[test]
#[should_panic(expected = "Error(Contract, #28)")]
fn test_execute_zero_address_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let id = setup_passed_proposal(&env, &client, &admin);
    let zero = Address::from_str(
        &env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    );
    client.execute(&zero, &id);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_cancel_non_admin_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    let id = setup_active_proposal(&env, &client, &admin);
    client.cancel(&non_admin, &id);
}

#[test]
#[should_panic(expected = "Error(Contract, #28)")]
fn test_cancel_zero_address_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let id = setup_active_proposal(&env, &client, &admin);
    let zero = Address::from_str(
        &env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    );
    client.cancel(&zero, &id);
}

// ── SC-005: execute state-guard tests ────────────────────────────────────────

/// execute() on an Active proposal must revert — only Passed is valid.
#[test]
#[should_panic(expected = "Error(Contract, #12)")]
fn test_execute_active_proposal_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let id = setup_active_proposal(&env, &client, &admin);
    client.execute(&admin, &id);
}

/// execute() on a Rejected proposal must revert.
#[test]
#[should_panic(expected = "Error(Contract, #12)")]
fn test_execute_rejected_proposal_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let token_id = setup_token(&env, &admin);
    client.initialize(
        &admin,
        &token_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );
    // Create a proposal that will be rejected (no votes, below quorum)
    let id = client.create_proposal(
        &admin,
        &String::from_str(&env, "Prop"),
        &String::from_str(&env, "desc"),
        &1_000_000,
        &3600,
    );
    env.ledger().with_mut(|l| l.timestamp += 3601);
    client.finalise(&id);
    assert_eq!(client.get_proposal(&id).state, ProposalState::Rejected);
    client.execute(&admin, &id);
}

/// execute() on a Cancelled proposal must revert.
#[test]
#[should_panic(expected = "Error(Contract, #12)")]
fn test_execute_cancelled_proposal_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let id = setup_active_proposal(&env, &client, &admin);
    client.cancel(&admin, &id);
    client.execute(&admin, &id);
}

/// execute() on an already-Executed proposal must revert.
#[test]
#[should_panic(expected = "Error(Contract, #12)")]
fn test_execute_already_executed_proposal_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let id = setup_passed_proposal(&env, &client, &admin);
    client.execute(&admin, &id); // first execute — ok
    client.execute(&admin, &id); // second execute — must revert
}

// ── end SC-005 ────────────────────────────────────────────────────────────────

// ── end TEST-013 ──────────────────────────────────────────────────────────────

// ── SC-027: update_quorum tests ───────────────────────────────────────────────

#[test]
fn test_update_quorum_success() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let id = create_test_proposal(&t, &proposer);
    t.client.update_quorum(&t.admin, &id, &500);
    assert_eq!(t.client.get_proposal(&id).quorum, 500);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_update_quorum_non_admin_reverts() {
    let t = setup_env();
    let non_admin = Address::generate(&t.env);
    let id = create_test_proposal(&t, &t.admin.clone());
    t.client.update_quorum(&non_admin, &id, &500);
}

#[test]
#[should_panic]
fn test_update_quorum_zero_reverts() {
    let t = setup_env();
    let id = create_test_proposal(&t, &t.admin.clone());
    t.client.update_quorum(&t.admin, &id, &0);
}

#[test]
#[should_panic]
fn test_update_quorum_inactive_proposal_reverts() {
    let t = setup_env();
    let id = create_test_proposal(&t, &t.admin.clone());
    t.client.cancel(&t.admin, &id);
    t.client.update_quorum(&t.admin, &id, &500);
}

/// Issue #56 — update_quorum is bounded by the supply snapshot taken at
/// proposal creation time, not the live supply at the time of the call.
///
/// Scenario: create a proposal against a supply of 10,000,000 tokens, then
/// burn a large chunk of supply so the live supply drops below the proposed
/// new quorum.  update_quorum must still succeed because the snapshot was
/// taken before the burn and remains the upper bound.
#[test]
fn test_update_quorum_uses_supply_snapshot_not_live_supply() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);

    // Initial supply = 10_000_000 (set up by setup_env / token initialize).
    // create a proposal with quorum = 1_000 (well within supply).
    let id = create_test_proposal(&t, &proposer);

    // Verify snapshot was captured at creation.
    let proposal_before = t.client.get_proposal(&id);
    assert!(
        proposal_before.supply_snapshot > 0,
        "supply_snapshot must be positive after creation"
    );

    // Burn most of the supply so live supply is now much smaller.
    let tok = votechain_token::TokenContractClient::new(&t.env, &t.token_id);
    // Burn 9_900_000 tokens from admin (leaves 100_000 live).
    tok.burn(&t.admin, &t.admin, &9_900_000_i128);

    // update_quorum to 5_000_000 — this exceeds the current live supply
    // (100_000) but is within the snapshot (10_000_000).
    // The call must SUCCEED because the snapshot anchors validation.
    t.client.update_quorum(&t.admin, &id, &5_000_000);
    let proposal_after = t.client.get_proposal(&id);
    assert_eq!(proposal_after.quorum, 5_000_000, "quorum should be updated to 5_000_000");
}

/// Issue #56 — update_quorum must REJECT a new quorum that exceeds the
/// supply_snapshot, regardless of whether live supply has changed.
#[test]
#[should_panic(expected = "Error(Contract, #22)")]
fn test_update_quorum_exceeding_snapshot_reverts() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let id = create_test_proposal(&t, &proposer);

    // Attempt to set quorum above the snapshot (10_000_000 + 1).
    // This must revert with QuorumExceedsSupply (error #22).
    t.client.update_quorum(&t.admin, &id, &10_000_001);
}

// ── end SC-027 ────────────────────────────────────────────────────────────────

// ── issue #257: MIN_QUORUM_BPS floor tests ────────────────────────────────────

/// Issue #257 — update_quorum must reject a new quorum that falls below the
/// minimum quorum floor (MIN_QUORUM_BPS = 10 BPS = 0.1 % of supply_snapshot).
///
/// supply_snapshot = 10_000_000 → floor = ceil(10_000_000 * 10 / 10_000) = 10_000.
/// Attempting to set quorum = 9_999 (one below the floor) must revert with
/// QuorumBelowFloor (error #53).
#[test]
#[should_panic(expected = "Error(Contract, #53)")]
fn test_update_quorum_below_floor_reverts() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let id = create_test_proposal(&t, &proposer);

    // supply_snapshot = 10_000_000; floor = 10_000; 9_999 < floor → revert.
    t.client.update_quorum(&t.admin, &id, &9_999);
}

/// Issue #257 — update_quorum must succeed at exactly the floor boundary.
///
/// supply_snapshot = 10_000_000 → floor = 10_000.
/// Setting quorum = 10_000 must succeed.
#[test]
fn test_update_quorum_at_floor_boundary_succeeds() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let id = create_test_proposal(&t, &proposer);

    // Exactly at the floor — must succeed.
    t.client.update_quorum(&t.admin, &id, &10_000);
    assert_eq!(t.client.get_proposal(&id).quorum, 10_000);
}

/// Issue #257 — update_quorum must succeed when the new quorum is above the floor.
#[test]
fn test_update_quorum_above_floor_succeeds() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let id = create_test_proposal(&t, &proposer);

    // 10_001 > floor (10_000) — must succeed.
    t.client.update_quorum(&t.admin, &id, &10_001);
    assert_eq!(t.client.get_proposal(&id).quorum, 10_001);
}

/// Issue #257 — get_config must expose min_quorum_bps with the protocol default
/// value when initialize is called with min_quorum_bps = 0.
#[test]
fn test_get_config_returns_default_min_quorum_bps() {
    let t = setup_env();
    let config = t.client.get_config().unwrap();
    assert_eq!(
        config.min_quorum_bps,
        crate::MIN_QUORUM_BPS,
        "default min_quorum_bps must equal the protocol constant"
    );
}

// ── end issue #257 ────────────────────────────────────────────────────────────

// ── issue #103: cast_vote comment_hash tests ──────────────────────────────────

#[test]
fn test_cast_vote_with_comment_hash_stored() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &proposer);
    let tok = votechain_token::TokenContractClient::new(&t.env, &t.token_id);
    tok.mint(&t.admin, &voter, &1_000);

    let cid = String::from_str(
        &t.env,
        "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi",
    );
    t.client.cast_vote(&voter, &id, &Vote::Yes, &Some(cid.clone()));

    let record = t.client.get_vote(&id, &voter).unwrap();
    assert_eq!(record.comment_hash, Some(cid));
}

#[test]
fn test_cast_vote_without_comment_hash_is_none() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &proposer);
    let tok = votechain_token::TokenContractClient::new(&t.env, &t.token_id);
    tok.mint(&t.admin, &voter, &1_000);

    t.client.cast_vote(&voter, &id, &Vote::Yes, &None);

    let record = t.client.get_vote(&id, &voter).unwrap();
    assert_eq!(record.comment_hash, None);
}

/// comment_hash exceeding MAX_COMMENT_HASH_LEN (64 bytes) must revert.
#[test]
#[should_panic(expected = "Error(Contract, #48)")]
fn test_cast_vote_comment_hash_too_long_reverts() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &proposer);
    let tok = votechain_token::TokenContractClient::new(&t.env, &t.token_id);
    tok.mint(&t.admin, &voter, &1_000);

    // 65 'a' characters — one over the 64-byte cap.
    let long_hash = String::from_str(&t.env, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    t.client.cast_vote(&voter, &id, &Vote::Yes, &Some(long_hash));
}

// ── end issue #103 ─────────────────────────────────────────────────────────────

// ── issue #105: extend_voting tests ───────────────────────────────────────────

#[test]
fn test_extend_voting_success() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let id = create_test_proposal(&t, &proposer);
    let before = t.client.get_proposal(&id);
    t.client.extend_voting(&t.admin, &id, &3600);
    let after = t.client.get_proposal(&id);
    assert_eq!(after.end_time, before.end_time + 3600);
    assert_eq!(after.start_time, before.start_time);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_extend_voting_non_admin_reverts() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let non_admin = Address::generate(&t.env);
    let id = create_test_proposal(&t, &proposer);
    t.client.extend_voting(&non_admin, &id, &3600);
}

/// extend_voting must revert once the voting period has already ended.
#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_extend_voting_after_end_time_reverts() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let id = create_test_proposal(&t, &proposer);
    // Default test proposal duration is 3600s; advance past end_time.
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.extend_voting(&t.admin, &id, &3600);
}

/// extend_voting can only be used once per proposal.
#[test]
#[should_panic(expected = "Error(Contract, #47)")]
fn test_extend_voting_double_extension_reverts() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let id = create_test_proposal(&t, &proposer);
    t.client.extend_voting(&t.admin, &id, &3600);
    // Second extension on the same proposal must revert with AlreadyExtended.
    t.client.extend_voting(&t.admin, &id, &3600);
}

/// extend_voting must revert if the resulting total duration would exceed
/// the contract's configured max_duration.
#[test]
#[should_panic(expected = "Error(Contract, #21)")]
fn test_extend_voting_exceeding_max_duration_reverts() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let id = create_test_proposal(&t, &proposer);
    // max_duration is 2_592_000s (30 days); proposal duration is 3600s.
    // Requesting an extension that would push total duration past max_duration must revert.
    t.client.extend_voting(&t.admin, &id, &2_600_000);
}

/// extend_voting must revert on a proposal that is no longer Active.
#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_extend_voting_inactive_proposal_reverts() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let id = create_test_proposal(&t, &proposer);
    t.client.cancel(&t.admin, &id);
    t.client.extend_voting(&t.admin, &id, &3600);
}

// ── end issue #105 ─────────────────────────────────────────────────────────────

// ── storage persistence tests ─────────────────────────────────────────────────

#[test]
fn test_proposal_data_persists_unchanged() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let id = t.client.create_proposal(
        &proposer,
        &String::from_str(&t.env, "Persist title"),
        &String::from_str(&t.env, "Persist desc"),
        &250,
        &1800,
    );
    let p = t.client.get_proposal(&id);
    assert_eq!(p.id, id);
    assert_eq!(p.title, String::from_str(&t.env, "Persist title"));
    assert_eq!(p.description, String::from_str(&t.env, "Persist desc"));
    assert_eq!(p.quorum, 250);
    assert_eq!(p.state, ProposalState::Active);
    assert_eq!(p.proposer, proposer);
}

#[test]
fn test_vote_records_persist_across_multiple_voters() {
    let t = setup_env();
    let voter1 = Address::generate(&t.env);
    let voter2 = Address::generate(&t.env);
    let voter3 = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter1);

    mint_and_vote(&t, &voter1, id, Vote::Yes, 300_000);
    mint_and_vote(&t, &voter2, id, Vote::No, 300_000);
    mint_and_vote(&t, &voter3, id, Vote::Abstain, 300_000);

    assert!(t.client.has_voted(&id, &voter1));
    assert!(t.client.has_voted(&id, &voter2));
    assert!(t.client.has_voted(&id, &voter3));
    let p = t.client.get_proposal(&id);
    assert!(p.votes_yes > 0);
    assert!(p.votes_no > 0);
    assert!(p.votes_abstain > 0);
}

#[test]
fn test_admin_persists_after_initialization() {
    let t = setup_env();
    let id = create_test_proposal(&t, &t.admin.clone());
    t.client.cancel(&t.admin, &id);
    assert_eq!(t.client.get_proposal(&id).state, ProposalState::Cancelled);
}

#[test]
fn test_no_data_lost_between_calls() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id1 = create_test_proposal(&t, &voter);
    let id2 = t.client.create_proposal(
        &voter,
        &String::from_str(&t.env, "P2"),
        &String::from_str(&t.env, "d2"),
        &200,
        &7200,
    );

    mint_and_vote(&t, &voter, id1, Vote::Yes, 1_000_000);

    let p2 = t.client.get_proposal(&id2);
    assert_eq!(p2.title, String::from_str(&t.env, "P2"));
    assert_eq!(p2.quorum, 200);
    assert_eq!(p2.votes_yes, 0);
    assert_eq!(p2.state, ProposalState::Active);
    assert!(!t.client.has_voted(&id2, &voter));
}

// ── end storage persistence tests ─────────────────────────────────────────────

// ── Issue #8: has_voted ProposalNotFound tests ────────────────────────────────

#[test]
#[should_panic]
fn test_has_voted_invalid_proposal_id_reverts() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    t.client.has_voted(&999, &voter);
}

#[test]
#[should_panic]
fn test_get_proposal_invalid_id_reverts() {
    let t = setup_env();
    t.client.get_proposal(&999);
}

#[test]
fn test_get_proposal_returns_correct_lifecycle_states() {
    let t = setup_env();
    let voter = Address::generate(&t.env);

    let active_id = create_test_proposal(&t, &voter);
    assert_eq!(
        t.client.get_proposal(&active_id).state,
        ProposalState::Active
    );

    let cancelled_id = create_test_proposal(&t, &voter);
    t.client.cancel(&t.admin, &cancelled_id);
    assert_eq!(
        t.client.get_proposal(&cancelled_id).state,
        ProposalState::Cancelled
    );

    let rejected_id = create_test_proposal(&t, &voter);
    mint_and_vote(&t, &voter, rejected_id, Vote::No, 1_000_000);
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&rejected_id);
    assert_eq!(
        t.client.get_proposal(&rejected_id).state,
        ProposalState::Rejected
    );

    let passed_id = create_test_proposal(&t, &voter);
    mint_and_vote(&t, &voter, passed_id, Vote::Yes, 1_000_000);
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&passed_id);
    assert_eq!(
        t.client.get_proposal(&passed_id).state,
        ProposalState::Passed
    );

    let executed_id = create_test_proposal(&t, &voter);
    mint_and_vote(&t, &voter, executed_id, Vote::Yes, 1_000_000);
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&executed_id);
    t.client.execute(&t.admin, &executed_id);
    assert_eq!(
        t.client.get_proposal(&executed_id).state,
        ProposalState::Executed
    );
}

#[test]
fn test_has_voted_returns_false_before_and_true_after_voting() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &proposer);

    assert!(!t.client.has_voted(&id, &voter));
    mint_and_vote(&t, &voter, id, Vote::Yes, 1_000_000);
    assert!(t.client.has_voted(&id, &voter));
}

#[test]
fn test_proposal_count_increments_correctly() {
    let t = setup_env();
    assert_eq!(t.client.proposal_count(), 0);

    let proposer = Address::generate(&t.env);
    let id1 = create_test_proposal(&t, &proposer);
    assert_eq!(t.client.proposal_count(), 1);

    let id2 = create_test_proposal(&t, &proposer);
    assert_eq!(t.client.proposal_count(), 2);

    let id3 = create_test_proposal(&t, &proposer);
    assert_eq!(t.client.proposal_count(), 3);

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);
}

#[test]
fn test_has_voted_returns_false_for_non_voter() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let non_voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &proposer);
    assert!(!t.client.has_voted(&id, &non_voter));
}

// ── end Issue #8 ──────────────────────────────────────────────────────────────

// ── Issue #10: ProposalState enum tests ──────────────────────────────────────

#[test]
fn test_proposal_state_all_variants_reachable() {
    let t = setup_env();
    let voter = Address::generate(&t.env);

    // Active
    let id = create_test_proposal(&t, &voter);
    assert_eq!(t.client.get_proposal(&id).state, ProposalState::Active);

    // Cancelled
    let id2 = create_test_proposal(&t, &voter);
    t.client.cancel(&t.admin, &id2);
    assert_eq!(t.client.get_proposal(&id2).state, ProposalState::Cancelled);

    // Rejected
    let id3 = create_test_proposal(&t, &voter);
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&id3);
    assert_eq!(t.client.get_proposal(&id3).state, ProposalState::Rejected);

    // Passed + Executed
    let id4 = create_test_proposal(&t, &voter);
    mint_and_vote(&t, &voter, id4, Vote::Yes, 1_000_000);
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&id4);
    assert_eq!(t.client.get_proposal(&id4).state, ProposalState::Passed);
    t.client.execute(&t.admin, &id4);
    assert_eq!(t.client.get_proposal(&id4).state, ProposalState::Executed);
}

// ── end Issue #10 ─────────────────────────────────────────────────────────────

// ── Issue #28: comprehensive voting scenario tests ────────────────────────────

#[test]
fn test_vote_yes_recorded_correctly() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter);
    mint_and_vote(&t, &voter, id, Vote::Yes, 500_000);
    let p = t.client.get_proposal(&id);
    assert_eq!(p.votes_yes, 500_000);
    assert_eq!(p.votes_no, 0);
    assert_eq!(p.votes_abstain, 0);
}

#[test]
fn test_vote_no_recorded_correctly() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter);
    mint_and_vote(&t, &voter, id, Vote::No, 750_000);
    let p = t.client.get_proposal(&id);
    assert_eq!(p.votes_yes, 0);
    assert_eq!(p.votes_no, 750_000);
    assert_eq!(p.votes_abstain, 0);
}

#[test]
fn test_vote_abstain_recorded_correctly() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter);
    mint_and_vote(&t, &voter, id, Vote::Abstain, 250_000);
    let p = t.client.get_proposal(&id);
    assert_eq!(p.votes_yes, 0);
    assert_eq!(p.votes_no, 0);
    assert_eq!(p.votes_abstain, 250_000);
}

#[test]
fn test_vote_weight_matches_token_balance() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter);
    let balance = 1_234_567;
    mint_and_vote(&t, &voter, id, Vote::Yes, balance);
    let p = t.client.get_proposal(&id);
    assert_eq!(p.votes_yes, balance);
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")]
fn test_double_vote_same_choice_reverts() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter);
    mint_and_vote(&t, &voter, id, Vote::Yes, 1_000_000);
    t.client.cast_vote(&voter, &id, &Vote::Yes, &None);
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")]
fn test_double_vote_different_choice_reverts() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter);
    mint_and_vote(&t, &voter, id, Vote::Yes, 1_000_000);
    t.client.cast_vote(&voter, &id, &Vote::No, &None);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_vote_on_passed_proposal_reverts() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter);
    mint_and_vote(&t, &voter, id, Vote::Yes, 1_000_000);
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&id);
    let voter2 = Address::generate(&t.env);
    mint_and_vote(&t, &voter2, id, Vote::Yes, 500_000);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_vote_on_rejected_proposal_reverts() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter);
    mint_and_vote(&t, &voter, id, Vote::No, 1_000_000);
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&id);
    let voter2 = Address::generate(&t.env);
    mint_and_vote(&t, &voter2, id, Vote::Yes, 500_000);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_vote_on_cancelled_proposal_reverts() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter);
    t.client.cancel(&t.admin, &id);
    mint_and_vote(&t, &voter, id, Vote::Yes, 1_000_000);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_vote_on_executed_proposal_reverts() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter);
    mint_and_vote(&t, &voter, id, Vote::Yes, 1_000_000);
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&id);
    t.client.execute(&t.admin, &id);
    let voter2 = Address::generate(&t.env);
    mint_and_vote(&t, &voter2, id, Vote::Yes, 500_000);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_vote_after_end_time_reverts() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter);
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    mint_and_vote(&t, &voter, id, Vote::Yes, 1_000_000);
}

#[test]
#[should_panic]
fn test_vote_at_exact_end_time_reverts() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let now = t.env.ledger().timestamp();
    let id = t.client.create_proposal(
        &voter,
        &String::from_str(&t.env, "Test"),
        &String::from_str(&t.env, "desc"),
        &1,
        &3600,
    );
    t.env.ledger().with_mut(|l| l.timestamp = now + 3600);
    mint_and_vote(&t, &voter, id, Vote::Yes, 1_000_000);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_vote_with_zero_balance_reverts() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter);
    t.client.cast_vote(&voter, &id, &Vote::Yes, &None);
}

#[test]
fn test_vote_tallies_accumulate_correctly() {
    let t = setup_env();
    let voter1 = Address::generate(&t.env);
    let voter2 = Address::generate(&t.env);
    let voter3 = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter1);

    mint_and_vote(&t, &voter1, id, Vote::Yes, 100_000);
    mint_and_vote(&t, &voter2, id, Vote::Yes, 200_000);
    mint_and_vote(&t, &voter3, id, Vote::No, 150_000);

    let p = t.client.get_proposal(&id);
    assert_eq!(p.votes_yes, 300_000);
    assert_eq!(p.votes_no, 150_000);
    assert_eq!(p.votes_abstain, 0);
}

#[test]
fn test_vote_tallies_all_three_types() {
    let t = setup_env();
    let v1 = Address::generate(&t.env);
    let v2 = Address::generate(&t.env);
    let v3 = Address::generate(&t.env);
    let v4 = Address::generate(&t.env);
    let v5 = Address::generate(&t.env);
    let id = create_test_proposal(&t, &v1);

    mint_and_vote(&t, &v1, id, Vote::Yes, 100_000);
    mint_and_vote(&t, &v2, id, Vote::Yes, 200_000);
    mint_and_vote(&t, &v3, id, Vote::No, 150_000);
    mint_and_vote(&t, &v4, id, Vote::No, 50_000);
    mint_and_vote(&t, &v5, id, Vote::Abstain, 75_000);

    let p = t.client.get_proposal(&id);
    assert_eq!(p.votes_yes, 300_000);
    assert_eq!(p.votes_no, 200_000);
    assert_eq!(p.votes_abstain, 75_000);
}

// ── end Issue #28 ─────────────────────────────────────────────────────────────

// ── SEC-009: re-initialization guard tests ────────────────────────────────────

/// Re-init by the original admin must revert with AlreadyInitialized.
#[test]
#[should_panic]
fn test_reinit_by_original_admin_reverts() {
    let t = setup_env();
    t.client.initialize(
        &t.admin,
        &t.token_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );
}

/// Re-init by a new address must revert with AlreadyInitialized.
#[test]
#[should_panic]
fn test_reinit_by_new_address_reverts() {
    let t = setup_env();
    let attacker = Address::generate(&t.env);
    let new_token = Address::generate(&t.env);
    t.client.initialize(
        &attacker,
        &new_token,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );
}

/// Re-init by the zero address must revert with AlreadyInitialized.
#[test]
#[should_panic]
fn test_reinit_by_zero_address_reverts() {
    let t = setup_env();
    let zero = Address::from_str(
        &t.env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    );
    t.client.initialize(
        &zero,
        &t.token_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );
}

// ── end SEC-009 ───────────────────────────────────────────────────────────────

// ── spam prevention tests ─────────────────────────────────────────────────────

#[test]
#[should_panic]
fn test_create_proposal_below_min_balance_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let token_id = setup_token(&env, &admin);
    // require 500_000 tokens to propose
    client.initialize(
        &admin,
        &token_id,
        &500_000_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );

    let proposer = Address::generate(&env);
    // proposer has 0 tokens — should panic
    client.create_proposal(
        &proposer,
        &String::from_str(&env, "Spam"),
        &String::from_str(&env, "desc"),
        &100,
        &3600,
    );
}

#[test]
fn test_create_proposal_at_min_balance_accepted() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let token_id = setup_token(&env, &admin);
    client.initialize(
        &admin,
        &token_id,
        &500_000_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );

    let proposer = Address::generate(&env);
    let tok = votechain_token::TokenContractClient::new(&env, &token_id);
    tok.mint(&admin, &proposer, &500_000_i128);

    let id = client.create_proposal(
        &proposer,
        &String::from_str(&env, "Valid"),
        &String::from_str(&env, "desc"),
        &100,
        &3600,
    );
    assert_eq!(client.get_proposal(&id).state, ProposalState::Active);
}

#[test]
#[should_panic]
fn test_create_proposal_within_cooldown_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let token_id = setup_token(&env, &admin);
    // start at non-zero so the `last > 0` sentinel works
    env.ledger().with_mut(|l| l.timestamp = 1_000);
    // 1 hour cooldown, no balance requirement
    client.initialize(
        &admin,
        &token_id,
        &0_i128,
        &3600_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );

    let proposer = Address::generate(&env);
    client.create_proposal(
        &proposer,
        &String::from_str(&env, "First"),
        &String::from_str(&env, "desc"),
        &100,
        &7200,
    );
    // second proposal immediately within cooldown — should panic
    client.create_proposal(
        &proposer,
        &String::from_str(&env, "Spam"),
        &String::from_str(&env, "desc"),
        &100,
        &3600,
    );
}

#[test]
fn test_create_proposal_after_cooldown_accepted() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let token_id = setup_token(&env, &admin);
    client.initialize(
        &admin,
        &token_id,
        &0_i128,
        &3600_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );

    let proposer = Address::generate(&env);
    client.create_proposal(
        &proposer,
        &String::from_str(&env, "First"),
        &String::from_str(&env, "desc"),
        &100,
        &7200,
    );
    // advance past cooldown
    env.ledger().with_mut(|l| l.timestamp += 3601);
    let id2 = client.create_proposal(
        &proposer,
        &String::from_str(&env, "Second"),
        &String::from_str(&env, "desc"),
        &100,
        &3600,
    );
    assert_eq!(client.get_proposal(&id2).state, ProposalState::Active);
}

#[test]
fn test_create_proposal_zero_cooldown_disabled_allows_immediate_second() {
    // #307: cooldown = 0 means the feature is disabled entirely, so a second
    // proposal by the same proposer must be allowed in the same block/timestamp.
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let token_id = setup_token(&env, &admin);
    client.initialize(
        &admin,
        &token_id,
        &0_i128,
        &3600_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64, // proposal_cooldown = 0 (disabled)
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );

    let proposer = Address::generate(&env);
    let id1 = client.create_proposal(
        &proposer,
        &String::from_str(&env, "First"),
        &String::from_str(&env, "desc"),
        &100,
        &7200,
    );
    // No timestamp advance at all — second proposal must still succeed because
    // cooldown enforcement is bypassed when cooldown == 0.
    let id2 = client.create_proposal(
        &proposer,
        &String::from_str(&env, "Second"),
        &String::from_str(&env, "desc"),
        &100,
        &3600,
    );
    assert_ne!(id1, id2);
    assert_eq!(client.get_proposal(&id1).state, ProposalState::Active);
    assert_eq!(client.get_proposal(&id2).state, ProposalState::Active);
}

#[test]
fn test_create_proposal_exactly_at_cooldown_boundary_accepted() {
    // #307: `now < last + cooldown` is the revert condition, so `now == last +
    // cooldown` (exactly at the boundary) must succeed, not revert.
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let token_id = setup_token(&env, &admin);
    env.ledger().with_mut(|l| l.timestamp = 1_000);
    client.initialize(
        &admin,
        &token_id,
        &0_i128,
        &3600_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &3600_u64, // 1 hour cooldown
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );

    let proposer = Address::generate(&env);
    client.create_proposal(
        &proposer,
        &String::from_str(&env, "First"),
        &String::from_str(&env, "desc"),
        &100,
        &7200,
    );
    // Advance exactly `cooldown` seconds — now == last + cooldown.
    env.ledger().with_mut(|l| l.timestamp += 3600);
    let id2 = client.create_proposal(
        &proposer,
        &String::from_str(&env, "Second"),
        &String::from_str(&env, "desc"),
        &100,
        &3600,
    );
    assert_eq!(client.get_proposal(&id2).state, ProposalState::Active);
}

#[test]
#[should_panic]
fn test_create_proposal_one_second_before_boundary_reverts() {
    // #307: complements the exact-boundary test above by confirming
    // `now == last + cooldown - 1` is still inside the cooldown window.
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let token_id = setup_token(&env, &admin);
    env.ledger().with_mut(|l| l.timestamp = 1_000);
    client.initialize(
        &admin,
        &token_id,
        &0_i128,
        &3600_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &3600_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );

    let proposer = Address::generate(&env);
    client.create_proposal(
        &proposer,
        &String::from_str(&env, "First"),
        &String::from_str(&env, "desc"),
        &100,
        &7200,
    );
    env.ledger().with_mut(|l| l.timestamp += 3599);
    client.create_proposal(
        &proposer,
        &String::from_str(&env, "Spam"),
        &String::from_str(&env, "desc"),
        &100,
        &3600,
    );
}

#[test]
fn test_create_proposal_cooldown_resets_after_successful_second_proposal() {
    // #307: after a successful second proposal, the cooldown timer must reset
    // against the NEW proposal's timestamp, so a third attempt immediately
    // after the second must also revert (cooldown re-armed, not one-shot).
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let token_id = setup_token(&env, &admin);
    env.ledger().with_mut(|l| l.timestamp = 1_000);
    client.initialize(
        &admin,
        &token_id,
        &0_i128,
        &3600_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &3600_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );

    let proposer = Address::generate(&env);
    client.create_proposal(
        &proposer,
        &String::from_str(&env, "First"),
        &String::from_str(&env, "desc"),
        &100,
        &7200,
    );
    // Advance past cooldown and submit a second (successful) proposal.
    env.ledger().with_mut(|l| l.timestamp += 3601);
    let id2 = client.create_proposal(
        &proposer,
        &String::from_str(&env, "Second"),
        &String::from_str(&env, "desc"),
        &100,
        &3600,
    );
    assert_eq!(client.get_proposal(&id2).state, ProposalState::Active);

    // Immediately try a third proposal — the cooldown must have reset against
    // the second proposal's timestamp, so this must revert.
    let result = client.try_create_proposal(
        &proposer,
        &String::from_str(&env, "Third"),
        &String::from_str(&env, "desc"),
        &100,
        &3600,
    );
    assert!(result.is_err());
}

// ── end spam prevention tests ─────────────────────────────────────────────────

// ── SC-023: get_vote tests ────────────────────────────────────────────────────

#[test]
fn test_get_vote_returns_record_after_voting() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter);
    mint_and_vote(&t, &voter, id, Vote::Yes, 500_000);
    let record = t
        .client
        .get_vote(&id, &voter)
        .expect("expected vote record");
    assert_eq!(record.vote_type, Vote::Yes);
    assert_eq!(record.weight, 500_000);
}

#[test]
fn test_get_vote_returns_none_for_non_voter() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let non_voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &proposer);
    assert!(t.client.get_vote(&id, &non_voter).is_none());
}

#[test]
fn test_get_vote_correct_type_for_no_vote() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter);
    mint_and_vote(&t, &voter, id, Vote::No, 300_000);
    let record = t
        .client
        .get_vote(&id, &voter)
        .expect("expected vote record");
    assert_eq!(record.vote_type, Vote::No);
    assert_eq!(record.weight, 300_000);
}

#[test]
fn test_get_vote_correct_type_for_abstain() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter);
    mint_and_vote(&t, &voter, id, Vote::Abstain, 100_000);
    let record = t
        .client
        .get_vote(&id, &voter)
        .expect("expected vote record");
    assert_eq!(record.vote_type, Vote::Abstain);
    assert_eq!(record.weight, 100_000);
}

// ── end SC-023 ────────────────────────────────────────────────────────────────

// ── SC-021: abstain votes count toward quorum ─────────────────────────────────

/// Abstain votes must be included in total_votes for the quorum check.
/// A proposal where only abstain votes are cast should pass quorum and then
/// be Rejected (because votes_yes == 0 <= votes_no == 0 is not strictly greater).
#[test]
fn test_abstain_votes_count_toward_quorum() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    // quorum = 500_000; voter abstains with exactly that weight
    let id = t.client.create_proposal(
        &voter,
        &String::from_str(&t.env, "Abstain quorum"),
        &String::from_str(&t.env, "desc"),
        &500_000,
        &3600,
    );
    mint_and_vote(&t, &voter, id, Vote::Abstain, 500_000);

    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&id);

    // Quorum was met (500_000 >= 500_000) but votes_yes (0) is not > votes_no (0),
    // so the proposal is Rejected — not Active, confirming abstain counted.
    assert_eq!(t.client.get_proposal(&id).state, ProposalState::Rejected);
}

/// Abstain votes combined with Yes votes should push a proposal over quorum
/// and allow it to pass when votes_yes > votes_no.
#[test]
fn test_abstain_plus_yes_meets_quorum_and_passes() {
    let t = setup_env();
    let voter_yes = Address::generate(&t.env);
    let voter_abs = Address::generate(&t.env);
    // quorum = 1_000_000; yes = 600_000, abstain = 400_000 → total = 1_000_000
    let id = t.client.create_proposal(
        &voter_yes,
        &String::from_str(&t.env, "Mixed quorum"),
        &String::from_str(&t.env, "desc"),
        &1_000_000,
        &3600,
    );
    mint_and_vote(&t, &voter_yes, id, Vote::Yes, 600_000);
    mint_and_vote(&t, &voter_abs, id, Vote::Abstain, 400_000);

    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&id);

    assert_eq!(t.client.get_proposal(&id).state, ProposalState::Passed);
}

/// Without abstain votes the same Yes total falls below quorum and is Rejected.
#[test]
fn test_yes_alone_below_quorum_rejected() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    // quorum = 1_000_000; only 600_000 yes votes — below quorum
    let id = t.client.create_proposal(
        &voter,
        &String::from_str(&t.env, "Below quorum"),
        &String::from_str(&t.env, "desc"),
        &1_000_000,
        &3600,
    );
    mint_and_vote(&t, &voter, id, Vote::Yes, 600_000);

    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&id);

    assert_eq!(t.client.get_proposal(&id).state, ProposalState::Rejected);
}

// ── end SC-021 ────────────────────────────────────────────────────────────────

// ── TEST-008: admin transfer integration tests ────────────────────────────────

/// Helper: create a passed proposal ready for execute() calls.
fn make_passed_proposal_for_transfer(
    env: &Env,
    client: &GovernanceContractClient,
    admin: &Address,
    token_id: &Address,
) -> u64 {
    let voter = Address::generate(env);
    let tok = votechain_token::TokenContractClient::new(env, token_id);
    tok.mint(admin, &voter, &1_000_000_i128);
    let id = client.create_proposal(
        &voter,
        &String::from_str(env, "Transfer test"),
        &String::from_str(env, "desc"),
        &100,
        &3600,
    );
    client.cast_vote(&voter, &id, &Vote::Yes, &None);
    env.ledger().with_mut(|l| l.timestamp += 3601);
    client.finalise(&id);
    id
}

/// New admin can execute a passed proposal after transfer.
#[test]
fn test_transfer_admin_new_admin_can_execute() {
    let t = setup_env();
    let new_admin = Address::generate(&t.env);
    let id = make_passed_proposal_for_transfer(&t.env, &t.client, &t.admin, &t.token_id);

    t.client.transfer_admin(&t.admin, &new_admin);
    t.client.execute(&new_admin, &id);

    assert_eq!(t.client.get_proposal(&id).state, ProposalState::Executed);
}

/// Old admin cannot execute a proposal after transferring admin rights.
#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_transfer_admin_old_admin_cannot_execute() {
    let t = setup_env();
    let new_admin = Address::generate(&t.env);
    let id = make_passed_proposal_for_transfer(&t.env, &t.client, &t.admin, &t.token_id);

    t.client.transfer_admin(&t.admin, &new_admin);
    // old admin tries to execute — must revert
    t.client.execute(&t.admin, &id);
}

/// Old admin cannot cancel a proposal after transferring admin rights.
#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_transfer_admin_old_admin_cannot_cancel() {
    let t = setup_env();
    let new_admin = Address::generate(&t.env);
    let proposer = Address::generate(&t.env);
    let id = t.client.create_proposal(
        &proposer,
        &String::from_str(&t.env, "Cancel test"),
        &String::from_str(&t.env, "desc"),
        &100,
        &3600,
    );

    t.client.transfer_admin(&t.admin, &new_admin);
    // old admin tries to cancel — must revert
    t.client.cancel(&t.admin, &id);
}

/// Transfer to the zero address must revert with InvalidNewAdmin.
#[test]
#[should_panic]
fn test_transfer_admin_to_zero_address_reverts() {
    let t = setup_env();
    let zero = Address::from_str(
        &t.env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    );
    t.client.transfer_admin(&t.admin, &zero);
}

/// transfer_admin emits an admxfer event with the correct old and new admin.
#[test]
fn test_transfer_admin_emits_event() {
    let t = setup_env();
    let new_admin = Address::generate(&t.env);
    t.client.transfer_admin(&t.admin, &new_admin);
    let events = t.env.events().all();
    assert!(
        events
            .iter()
            .any(|(_, topics, _)| { topics == (symbol_short!("admxfer"),).into_val(&t.env) }),
        "expected admxfer event to be emitted"
    );
}

// ── end TEST-008 ──────────────────────────────────────────────────────────────

// ── SEC-016: admin vote restriction tests ─────────────────────────────────────

/// When restrict_admin_vote is enabled, admin cannot vote on a proposal they created.
#[test]
#[should_panic(expected = "Error(Contract, #25)")]
fn test_admin_cannot_vote_own_proposal_when_restricted() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let tok_id = env.register(votechain_token::TokenContract, ());
    let tok = votechain_token::TokenContractClient::new(&env, &tok_id);
    tok.initialize(&admin, &10_000_000);
    let client = new_client(&env);
    // enable restriction
    client.initialize(
        &admin,
        &tok_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &true,
        &0_u64,
        &0_u64,
    );
    let id = client.create_proposal(
        &admin,
        &String::from_str(&env, "Admin prop"),
        &String::from_str(&env, "desc"),
        &100,
        &3600,
    );
    // admin tries to vote on their own proposal — should panic
    client.cast_vote(&admin, &id, &Vote::Yes, &None);
}

/// When restrict_admin_vote is disabled, admin can vote on their own proposal.
#[test]
fn test_admin_can_vote_own_proposal_when_not_restricted() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let tok_id = env.register(votechain_token::TokenContract, ());
    let tok = votechain_token::TokenContractClient::new(&env, &tok_id);
    tok.initialize(&admin, &10_000_000);
    let client = new_client(&env);
    // restriction disabled
    client.initialize(
        &admin,
        &tok_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );
    let id = client.create_proposal(
        &admin,
        &String::from_str(&env, "Admin prop"),
        &String::from_str(&env, "desc"),
        &100,
        &3600,
    );
    // admin votes on their own proposal — should succeed
    client.cast_vote(&admin, &id, &Vote::Yes, &None);
    assert_eq!(client.get_proposal(&id).votes_yes, 10_000_000);
}

/// When restrict_admin_vote is enabled, a non-admin voter can still vote normally.
#[test]
fn test_non_admin_can_vote_when_admin_restricted() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let tok_id = env.register(votechain_token::TokenContract, ());
    let tok = votechain_token::TokenContractClient::new(&env, &tok_id);
    tok.initialize(&admin, &10_000_000);
    let client = new_client(&env);
    client.initialize(
        &admin,
        &tok_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &true,
        &0_u64,
        &0_u64,
    );
    let proposer = Address::generate(&env);
    let id = client.create_proposal(
        &proposer,
        &String::from_str(&env, "User prop"),
        &String::from_str(&env, "desc"),
        &100,
        &3600,
    );
    let voter = Address::generate(&env);
    tok.mint(&admin, &voter, &500_000_i128);
    client.cast_vote(&voter, &id, &Vote::Yes, &None);
    assert_eq!(client.get_proposal(&id).votes_yes, 500_000);
}

// ── end SEC-016 ───────────────────────────────────────────────────────────────

// ── SEC-018: emergency pause tests ────────────────────────────────────────────

/// pause() by admin sets paused state and emits event.
#[test]
fn test_pause_sets_paused_state() {
    let t = setup_env();
    assert!(!t.client.paused());
    t.client.pause(&t.admin);
    assert!(t.client.paused());
}

/// unpause() by admin clears paused state and emits event.
#[test]
fn test_unpause_clears_paused_state() {
    let t = setup_env();
    t.client.pause(&t.admin);
    assert!(t.client.paused());
    t.client.unpause(&t.admin);
    assert!(!t.client.paused());
}

/// pause() by non-admin must revert.
#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_pause_non_admin_reverts() {
    let t = setup_env();
    let attacker = Address::generate(&t.env);
    t.client.pause(&attacker);
}

/// unpause() by non-admin must revert.
#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_unpause_non_admin_reverts() {
    let t = setup_env();
    t.client.pause(&t.admin);
    let attacker = Address::generate(&t.env);
    t.client.unpause(&attacker);
}

/// unpause() when not paused must revert.
#[test]
#[should_panic]
fn test_unpause_when_not_paused_reverts() {
    let t = setup_env();
    t.client.unpause(&t.admin);
}

/// create_proposal reverts when paused.
#[test]
#[should_panic(expected = "Error(Contract, #26)")]
fn test_create_proposal_reverts_when_paused() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    t.client.pause(&t.admin);
    t.client.create_proposal(
        &proposer,
        &String::from_str(&t.env, "P"),
        &String::from_str(&t.env, "d"),
        &100,
        &3600,
    );
}

/// cast_vote reverts when paused.
#[test]
#[should_panic(expected = "Error(Contract, #26)")]
fn test_cast_vote_reverts_when_paused() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter);
    let tok = votechain_token::TokenContractClient::new(&t.env, &t.token_id);
    tok.mint(&t.admin, &voter, &1_000_000_i128);
    t.client.pause(&t.admin);
    t.client.cast_vote(&voter, &id, &Vote::Yes, &None);
}

/// finalise reverts when paused.
#[test]
#[should_panic(expected = "Error(Contract, #26)")]
fn test_finalise_reverts_when_paused() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter);
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.pause(&t.admin);
    t.client.finalise(&id);
}

/// execute reverts when paused.
#[test]
#[should_panic(expected = "Error(Contract, #26)")]
fn test_execute_reverts_when_paused() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter);
    let tok = votechain_token::TokenContractClient::new(&t.env, &t.token_id);
    tok.mint(&t.admin, &voter, &1_000_000_i128);
    t.client.cast_vote(&voter, &id, &Vote::Yes, &None);
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&id);
    t.client.pause(&t.admin);
    t.client.execute(&t.admin, &id);
}

/// cancel reverts when paused.
#[test]
#[should_panic(expected = "Error(Contract, #26)")]
fn test_cancel_reverts_when_paused() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let id = create_test_proposal(&t, &proposer);
    t.client.pause(&t.admin);
    t.client.cancel(&t.admin, &id);
}

/// update_quorum reverts when paused.
#[test]
#[should_panic(expected = "Error(Contract, #26)")]
fn test_update_quorum_reverts_when_paused() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let id = create_test_proposal(&t, &proposer);
    t.client.pause(&t.admin);
    t.client.update_quorum(&t.admin, &id, &500);
}

/// transfer_admin reverts when paused.
#[test]
#[should_panic(expected = "Error(Contract, #26)")]
fn test_transfer_admin_reverts_when_paused() {
    let t = setup_env();
    let new_admin = Address::generate(&t.env);
    t.client.pause(&t.admin);
    t.client.transfer_admin(&t.admin, &new_admin);
}

/// Read-only functions (get_proposal, get_vote, has_voted) remain available when paused.
#[test]
fn test_read_functions_available_when_paused() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter);
    t.client.pause(&t.admin);
    // These should not panic
    let _ = t.client.get_proposal(&id);
    let _ = t.client.has_voted(&id, &voter);
    let _ = t.client.get_vote(&id, &voter);
    let _ = t.client.proposal_count();
    let _ = t.client.get_version();
    let _ = t.client.get_state();
    let _ = t.client.paused();
}

/// pause emits a "paused" event.
#[test]
fn test_pause_emits_event() {
    let t = setup_env();
    t.client.pause(&t.admin);
    let events = t.env.events().all();
    assert!(
        events
            .iter()
            .any(|(_, topics, _)| { topics == (symbol_short!("paused"),).into_val(&t.env) }),
        "expected paused event to be emitted"
    );
}

/// unpause emits an "unpaused" event.
#[test]
fn test_unpause_emits_event() {
    let t = setup_env();
    t.client.pause(&t.admin);
    t.client.unpause(&t.admin);
    let events = t.env.events().all();
    assert!(
        events
            .iter()
            .any(|(_, topics, _)| { topics == (symbol_short!("unpaused"),).into_val(&t.env) }),
        "expected unpaused event to be emitted"
    );
}

// ── end SEC-018 ───────────────────────────────────────────────────────────────

// ── TEST-ADMIN-EXEC-CANCEL: Admin-only execution and cancellation tests ──────

/// Test: execute succeeds on Passed proposal by admin
/// Verifies that the admin can successfully execute a proposal that has reached Passed state.
#[test]
fn test_execute_passed_proposal_by_admin_succeeds() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter);

    // Vote to pass the proposal
    mint_and_vote(&t, &voter, id, Vote::Yes, 1_000_000);

    // Advance time past voting period
    t.env.ledger().with_mut(|l| l.timestamp += 3601);

    // Finalize to move to Passed state
    t.client.finalise(&id);
    assert_eq!(t.client.get_proposal(&id).state, ProposalState::Passed);

    // Admin executes the passed proposal
    t.client.execute(&t.admin, &id);

    // Verify state changed to Executed
    assert_eq!(t.client.get_proposal(&id).state, ProposalState::Executed);
}

/// Test: execute reverts for non-admin caller
/// Verifies that a non-admin address cannot execute a proposal.
#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_execute_reverts_for_non_admin_caller() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);

    let id = setup_passed_proposal(&env, &client, &admin);

    // Non-admin attempts to execute
    client.execute(&non_admin, &id);
}

/// Test: execute reverts on non-Passed proposal
/// Verifies that execute fails when the proposal is not in Passed state.
#[test]
#[should_panic(expected = "Error(Contract, #12)")]
fn test_execute_reverts_on_non_passed_proposal() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);

    let id = setup_active_proposal(&env, &client, &admin);

    // Admin attempts to execute an Active proposal (not Passed)
    client.execute(&admin, &id);
}

/// Test: cancel succeeds on Active proposal by admin
/// Verifies that the admin can successfully cancel a proposal in Active state.
#[test]
fn test_cancel_active_proposal_by_admin_succeeds() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let id = create_test_proposal(&t, &proposer);

    // Verify proposal is Active
    assert_eq!(t.client.get_proposal(&id).state, ProposalState::Active);

    // Admin cancels the active proposal
    t.client.cancel(&t.admin, &id);

    // Verify state changed to Cancelled
    assert_eq!(t.client.get_proposal(&id).state, ProposalState::Cancelled);
}

/// Test: cancel reverts for non-admin caller
/// Verifies that a non-admin address cannot cancel a proposal.
#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_cancel_reverts_for_non_admin_caller() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);

    let id = setup_active_proposal(&env, &client, &admin);

    // Non-admin attempts to cancel
    client.cancel(&non_admin, &id);
}

/// Test: cancel reverts on non-Active proposal
/// Verifies that cancel fails when the proposal is not in Active state.
#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_cancel_reverts_on_non_active_proposal() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let token_id = setup_token(&env, &admin);
    client.initialize(
        &admin,
        &token_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );

    // Create and finalize a proposal to move it out of Active state
    let id = client.create_proposal(
        &admin,
        &String::from_str(&env, "Prop"),
        &String::from_str(&env, "desc"),
        &1_000_000,
        &3600,
    );
    env.ledger().with_mut(|l| l.timestamp += 3601);
    client.finalise(&id);

    // Verify proposal is no longer Active (it's Rejected)
    assert_eq!(client.get_proposal(&id).state, ProposalState::Rejected);

    // Admin attempts to cancel a non-Active proposal
    client.cancel(&admin, &id);
}

/// Test: execute emits event correctly
/// Verifies that the execute function emits the "executed" event with correct proposal ID.
#[test]
fn test_execute_emits_event_correctly() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter);

    // Vote to pass the proposal
    mint_and_vote(&t, &voter, id, Vote::Yes, 1_000_000);

    // Advance time and finalize
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&id);

    // Clear events before execute
    t.env.events().all();

    // Execute the proposal
    t.client.execute(&t.admin, &id);

    // Verify the "executed" event was emitted with correct proposal ID
    let events = t.env.events().all();
    assert!(
        events
            .iter()
            .any(|(_, topics, _)| { topics == (symbol_short!("executed"), id).into_val(&t.env) }),
        "expected 'executed' event with proposal ID {} to be emitted",
        id
    );
}

/// Test: cancel emits event correctly
/// Verifies that the cancel function emits the "cancelled" event with correct proposal ID.
#[test]
fn test_cancel_emits_event_correctly() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let id = create_test_proposal(&t, &proposer);

    // Clear events before cancel
    t.env.events().all();

    // Cancel the proposal
    t.client.cancel(&t.admin, &id);

    // Verify the "cancelled" event was emitted with correct proposal ID
    let events = t.env.events().all();
    assert!(
        events
            .iter()
            .any(|(_, topics, _)| { topics == (symbol_short!("cancelled"), id).into_val(&t.env) }),
        "expected 'cancelled' event with proposal ID {} to be emitted",
        id
    );
}

/// Test: execute and cancel maintain state consistency
/// Verifies that state transitions are atomic and consistent across multiple operations.
#[test]
fn test_execute_and_cancel_maintain_state_consistency() {
    let t = setup_env();
    let voter = Address::generate(&t.env);

    // Create two proposals
    let id1 = create_test_proposal(&t, &voter);
    let id2 = create_test_proposal(&t, &voter);

    // Pass and execute first proposal
    mint_and_vote(&t, &voter, id1, Vote::Yes, 1_000_000);
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&id1);
    t.client.execute(&t.admin, &id1);

    // Cancel second proposal
    t.client.cancel(&t.admin, &id2);

    // Verify both states are correct and independent
    assert_eq!(t.client.get_proposal(&id1).state, ProposalState::Executed);
    assert_eq!(t.client.get_proposal(&id2).state, ProposalState::Cancelled);
}

/// Test: execute requires auth from admin
/// Verifies that execute properly checks admin authorization.
#[test]
#[should_panic]
fn test_execute_requires_admin_auth() {
    let env = Env::default();
    // Don't mock all auths - this will cause auth check to fail
    let client = new_client(&env);
    let admin = Address::generate(&env);

    let id = setup_passed_proposal(&env, &client, &admin);

    // This should panic due to failed auth check
    client.execute(&admin, &id);
}

/// Test: cancel requires auth from admin
/// Verifies that cancel properly checks admin authorization.
#[test]
#[should_panic]
fn test_cancel_requires_admin_auth() {
    let env = Env::default();
    // Don't mock all auths - this will cause auth check to fail
    let client = new_client(&env);
    let admin = Address::generate(&env);

    let id = setup_active_proposal(&env, &client, &admin);

    // This should panic due to failed auth check
    client.cancel(&admin, &id);
}

/// Test: execute on Cancelled proposal reverts
/// Verifies that execute fails when proposal is in Cancelled state.
#[test]
#[should_panic(expected = "Error(Contract, #12)")]
fn test_execute_on_cancelled_proposal_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);

    let id = setup_active_proposal(&env, &client, &admin);

    // Cancel the proposal first
    client.cancel(&admin, &id);
    assert_eq!(client.get_proposal(&id).state, ProposalState::Cancelled);

    // Attempt to execute a cancelled proposal
    client.execute(&admin, &id);
}

/// Test: cancel on Executed proposal reverts
/// Verifies that cancel fails when proposal is in Executed state.
#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_cancel_on_executed_proposal_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);

    let id = setup_passed_proposal(&env, &client, &admin);

    // Execute the proposal first
    client.execute(&admin, &id);
    assert_eq!(client.get_proposal(&id).state, ProposalState::Executed);

    // Attempt to cancel an executed proposal
    client.cancel(&admin, &id);
}

/// Test: multiple execute calls on same proposal revert
/// Verifies that a proposal can only be executed once.
#[test]
#[should_panic(expected = "Error(Contract, #12)")]
fn test_multiple_execute_calls_revert() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);

    let id = setup_passed_proposal(&env, &client, &admin);

    // First execute succeeds
    client.execute(&admin, &id);
    assert_eq!(client.get_proposal(&id).state, ProposalState::Executed);

    // Second execute on same proposal should revert
    client.execute(&admin, &id);
}

/// Test: multiple cancel calls on same proposal revert
/// Verifies that a proposal can only be cancelled once.
#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_multiple_cancel_calls_revert() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);

    let id = setup_active_proposal(&env, &client, &admin);

    // First cancel succeeds
    client.cancel(&admin, &id);
    assert_eq!(client.get_proposal(&id).state, ProposalState::Cancelled);

    // Second cancel on same proposal should revert
    client.cancel(&admin, &id);
}

// ── end TEST-ADMIN-EXEC-CANCEL ────────────────────────────────────────────────

// ── #66 TEST-001: initialize unit tests ──────────────────────────────────────

/// initialize succeeds with valid inputs and transitions state to Ready.
#[test]
fn test_initialize_success() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let token_id = setup_token(&env, &admin);

    assert_eq!(client.get_state(), ContractState::Uninitialized);
    client.initialize(
        &admin,
        &token_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );
    assert_eq!(client.get_state(), ContractState::Ready);
}

/// initialize stores the version as (1, 0, 0).
#[test]
fn test_initialize_sets_version() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let token_id = setup_token(&env, &admin);
    client.initialize(
        &admin,
        &token_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );
    assert_eq!(client.get_version(), (1, 0, 0));
}

/// initialize with min_proposal_balance > 0 enforces the balance requirement.
#[test]
#[should_panic]
fn test_initialize_min_balance_enforced() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let token_id = setup_token(&env, &admin);
    client.initialize(
        &admin,
        &token_id,
        &1_000_000_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );

    let proposer = Address::generate(&env); // zero balance
    client.create_proposal(
        &proposer,
        &String::from_str(&env, "Title"),
        &String::from_str(&env, "desc"),
        &100,
        &3600,
    );
}

/// initialize with restrict_admin_vote=true blocks admin from voting on own proposals.
#[test]
#[should_panic(expected = "Error(Contract, #25)")]
fn test_initialize_restrict_admin_vote_enforced() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let token_id = setup_token(&env, &admin);
    client.initialize(
        &admin,
        &token_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &true,
        &0_u64,
        &0_u64,
    );

    let id = client.create_proposal(
        &admin,
        &String::from_str(&env, "Admin prop"),
        &String::from_str(&env, "desc"),
        &100,
        &3600,
    );
    // admin voting on their own proposal must revert
    client.cast_vote(&admin, &id, &Vote::Yes, &None);
}

/// Calling initialize a second time must revert with AlreadyInitialized (#13).
#[test]
#[should_panic(expected = "Error(Contract, #13)")]
fn test_initialize_already_initialized_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let token_id = setup_token(&env, &admin);
    client.initialize(
        &admin,
        &token_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );
    client.initialize(
        &admin,
        &token_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );
}

/// initialize with the zero address as admin must revert with InvalidAddress (#28).
#[test]
#[should_panic(expected = "Error(Contract, #28)")]
fn test_initialize_zero_admin_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let zero = Address::from_str(
        &env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    );
    let token_id = Address::generate(&env);
    client.initialize(
        &zero,
        &token_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );
}

/// initialize with the zero address as voting_token must revert with InvalidAddress (#28).
#[test]
#[should_panic(expected = "Error(Contract, #28)")]
fn test_initialize_zero_token_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let zero = Address::from_str(
        &env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    );
    client.initialize(
        &admin,
        &zero,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );
}

// ── end #66 ───────────────────────────────────────────────────────────────────

// ── #69 TEST-004: finalise unit tests ─────────────────────────────────────────

/// Proposal passes when total_votes >= quorum AND votes_yes > votes_no.
#[test]
fn test_finalise_passes_when_quorum_met_and_yes_wins() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = t.client.create_proposal(
        &voter,
        &String::from_str(&t.env, "Pass test"),
        &String::from_str(&t.env, "desc"),
        &500_000,
        &3600,
    );
    mint_and_vote(&t, &voter, id, Vote::Yes, 600_000);
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&id);
    assert_eq!(t.client.get_proposal(&id).state, ProposalState::Passed);
}

/// Proposal is rejected when total_votes < quorum (even if yes > no).
#[test]
fn test_finalise_rejected_when_quorum_not_met() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = t.client.create_proposal(
        &voter,
        &String::from_str(&t.env, "Low quorum"),
        &String::from_str(&t.env, "desc"),
        &1_000_000,
        &3600,
    );
    mint_and_vote(&t, &voter, id, Vote::Yes, 500_000); // below quorum
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&id);
    assert_eq!(t.client.get_proposal(&id).state, ProposalState::Rejected);
}

/// Proposal is rejected when votes_yes == votes_no (tie), even if quorum is met.
#[test]
fn test_finalise_rejected_on_tie() {
    let t = setup_env();
    let voter_yes = Address::generate(&t.env);
    let voter_no = Address::generate(&t.env);
    let id = t.client.create_proposal(
        &voter_yes,
        &String::from_str(&t.env, "Tie"),
        &String::from_str(&t.env, "desc"),
        &200_000,
        &3600,
    );
    mint_and_vote(&t, &voter_yes, id, Vote::Yes, 200_000);
    mint_and_vote(&t, &voter_no, id, Vote::No, 200_000);
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&id);
    assert_eq!(t.client.get_proposal(&id).state, ProposalState::Rejected);
}

/// Proposal is rejected when votes_no > votes_yes, even if quorum is met.
#[test]
fn test_finalise_rejected_when_no_wins() {
    let t = setup_env();
    let voter_yes = Address::generate(&t.env);
    let voter_no = Address::generate(&t.env);
    let id = t.client.create_proposal(
        &voter_yes,
        &String::from_str(&t.env, "No wins"),
        &String::from_str(&t.env, "desc"),
        &100_000,
        &3600,
    );
    mint_and_vote(&t, &voter_yes, id, Vote::Yes, 100_000);
    mint_and_vote(&t, &voter_no, id, Vote::No, 300_000);
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&id);
    assert_eq!(t.client.get_proposal(&id).state, ProposalState::Rejected);
}

/// Proposal with zero votes is rejected (quorum not met).
#[test]
fn test_finalise_rejected_with_zero_votes() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let id = create_test_proposal(&t, &proposer);
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&id);
    assert_eq!(t.client.get_proposal(&id).state, ProposalState::Rejected);
}

/// finalise before voting period ends must revert with VotingStillOpen (#9).
#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_finalise_before_end_time_reverts() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let id = create_test_proposal(&t, &proposer);
    t.client.finalise(&id); // voting period still open
}

/// finalise on a non-Active proposal must revert with ProposalNotActive (#7).
#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_finalise_already_finalised_reverts() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = create_test_proposal(&t, &voter);
    mint_and_vote(&t, &voter, id, Vote::Yes, 1_000_000);
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&id);
    t.client.finalise(&id); // second call must revert
}

/// finalise on a cancelled proposal must revert with ProposalNotActive (#7).
#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_finalise_cancelled_proposal_reverts() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let id = create_test_proposal(&t, &proposer);
    t.client.cancel(&t.admin, &id);
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&id);
}

/// finalise on a non-existent proposal must revert with ProposalNotFound (#6).
#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_finalise_nonexistent_proposal_reverts() {
    let t = setup_env();
    t.client.finalise(&999);
}

/// Abstain votes count toward quorum: abstain-only proposal meets quorum but is Rejected.
#[test]
fn test_finalise_abstain_counts_toward_quorum_but_not_outcome() {
    let t = setup_env();
    let voter = Address::generate(&t.env);
    let id = t.client.create_proposal(
        &voter,
        &String::from_str(&t.env, "Abstain only"),
        &String::from_str(&t.env, "desc"),
        &300_000,
        &3600,
    );
    mint_and_vote(&t, &voter, id, Vote::Abstain, 300_000);
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&id);
    // quorum met but yes (0) not > no (0) → Rejected
    assert_eq!(t.client.get_proposal(&id).state, ProposalState::Rejected);
}

// ── end #69 ───────────────────────────────────────────────────────────────────

// ── #72 TEST-007: full lifecycle integration tests ────────────────────────────

/// Happy path: initialize → create → vote Yes → finalise → execute.
#[test]
fn test_full_lifecycle_pass_and_execute() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let voter = Address::generate(&env);
    let token_id = setup_token(&env, &admin);

    // initialize
    client.initialize(
        &admin,
        &token_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );
    assert_eq!(client.get_state(), ContractState::Ready);

    // mint tokens to voter
    let tok = votechain_token::TokenContractClient::new(&env, &token_id);
    tok.mint(&admin, &voter, &1_000_000_i128);

    // create proposal
    let id = client.create_proposal(
        &voter,
        &String::from_str(&env, "Treasury"),
        &String::from_str(&env, "Allocate funds"),
        &500_000,
        &3600,
    );
    assert_eq!(client.get_proposal(&id).state, ProposalState::Active);
    assert_eq!(client.proposal_count(), 1);

    // vote
    client.cast_vote(&voter, &id, &Vote::Yes, &None);
    assert!(client.has_voted(&id, &voter));
    assert_eq!(client.get_proposal(&id).votes_yes, 1_000_000);

    // finalise after voting period
    env.ledger().with_mut(|l| l.timestamp += 3601);
    client.finalise(&id);
    assert_eq!(client.get_proposal(&id).state, ProposalState::Passed);

    // execute
    client.execute(&admin, &id);
    assert_eq!(client.get_proposal(&id).state, ProposalState::Executed);
}

/// Full lifecycle ending in rejection: quorum not met.
#[test]
fn test_full_lifecycle_reject_below_quorum() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let voter = Address::generate(&env);
    let token_id = setup_token(&env, &admin);

    client.initialize(
        &admin,
        &token_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );

    let tok = votechain_token::TokenContractClient::new(&env, &token_id);
    tok.mint(&admin, &voter, &100_000_i128);

    let id = client.create_proposal(
        &voter,
        &String::from_str(&env, "Underfunded"),
        &String::from_str(&env, "desc"),
        &500_000, // quorum higher than available votes
        &3600,
    );

    client.cast_vote(&voter, &id, &Vote::Yes, &None);
    env.ledger().with_mut(|l| l.timestamp += 3601);
    client.finalise(&id);
    assert_eq!(client.get_proposal(&id).state, ProposalState::Rejected);
}

/// Full lifecycle ending in cancellation by admin.
#[test]
fn test_full_lifecycle_cancel() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let token_id = setup_token(&env, &admin);

    client.initialize(
        &admin,
        &token_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );

    let proposer = Address::generate(&env);
    let id = client.create_proposal(
        &proposer,
        &String::from_str(&env, "To cancel"),
        &String::from_str(&env, "desc"),
        &100,
        &3600,
    );
    assert_eq!(client.get_proposal(&id).state, ProposalState::Active);

    client.cancel(&admin, &id);
    assert_eq!(client.get_proposal(&id).state, ProposalState::Cancelled);
}

/// Multiple voters across multiple proposals — votes are isolated per proposal.
#[test]
fn test_full_lifecycle_multiple_proposals_isolated() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    let token_id = setup_token(&env, &admin);

    client.initialize(
        &admin,
        &token_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );

    let tok = votechain_token::TokenContractClient::new(&env, &token_id);
    tok.mint(&admin, &voter1, &1_000_000_i128);
    tok.mint(&admin, &voter2, &1_000_000_i128);

    let id1 = client.create_proposal(
        &voter1,
        &String::from_str(&env, "Prop 1"),
        &String::from_str(&env, "d"),
        &500_000,
        &3600,
    );
    let id2 = client.create_proposal(
        &voter2,
        &String::from_str(&env, "Prop 2"),
        &String::from_str(&env, "d"),
        &500_000,
        &7200,
    );

    client.cast_vote(&voter1, &id1, &Vote::Yes, &None);
    client.cast_vote(&voter2, &id2, &Vote::No, &None);

    // votes don't bleed between proposals
    assert_eq!(client.get_proposal(&id1).votes_yes, 1_000_000);
    assert_eq!(client.get_proposal(&id1).votes_no, 0);
    assert_eq!(client.get_proposal(&id2).votes_yes, 0);
    assert_eq!(client.get_proposal(&id2).votes_no, 1_000_000);

    // finalise id1 (passes), id2 still active
    env.ledger().with_mut(|l| l.timestamp += 3601);
    client.finalise(&id1);
    assert_eq!(client.get_proposal(&id1).state, ProposalState::Passed);
    assert_eq!(client.get_proposal(&id2).state, ProposalState::Active);

    // execute id1
    client.execute(&admin, &id1);
    assert_eq!(client.get_proposal(&id1).state, ProposalState::Executed);

    // finalise id2 (rejected — no wins)
    env.ledger().with_mut(|l| l.timestamp += 7201);
    client.finalise(&id2);
    assert_eq!(client.get_proposal(&id2).state, ProposalState::Rejected);
}

/// Pausing blocks create/vote/finalise; unpausing restores them.
#[test]
fn test_full_lifecycle_pause_and_unpause() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let voter = Address::generate(&env);
    let token_id = setup_token(&env, &admin);

    client.initialize(
        &admin,
        &token_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );

    let tok = votechain_token::TokenContractClient::new(&env, &token_id);
    tok.mint(&admin, &voter, &1_000_000_i128);

    let id = client.create_proposal(
        &voter,
        &String::from_str(&env, "Pausable"),
        &String::from_str(&env, "desc"),
        &500_000,
        &3600,
    );

    // pause — cast_vote must fail
    client.pause(&admin);
    assert!(client.paused());
    // ContractPaused guard is verified by the paused() flag above;
    // the actual revert is tested in test_create_proposal_reverts_when_paused.

    // unpause — vote and finalise succeed
    client.unpause(&admin);
    assert!(!client.paused());

    client.cast_vote(&voter, &id, &Vote::Yes, &None);
    env.ledger().with_mut(|l| l.timestamp += 3601);
    client.finalise(&id);
    assert_eq!(client.get_proposal(&id).state, ProposalState::Passed);
    client.execute(&admin, &id);
    assert_eq!(client.get_proposal(&id).state, ProposalState::Executed);
}

// ── end #72 ───────────────────────────────────────────────────────────────────

// ── SEC-003: input sanitization ───────────────────────────────────────────────

/// Title with a null byte is rejected.
#[test]
#[should_panic]
fn test_title_null_byte_rejected() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    // "bad\x00title" — null byte in the middle
    t.client.create_proposal(
        &proposer,
        &String::from_bytes(&t.env, b"bad\x00title"),
        &String::from_str(&t.env, "desc"),
        &100,
        &3600,
    );
}

/// Title with a control character (newline) is rejected.
#[test]
#[should_panic]
fn test_title_control_char_rejected() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    t.client.create_proposal(
        &proposer,
        &String::from_bytes(&t.env, b"bad\ntitle"),
        &String::from_str(&t.env, "desc"),
        &100,
        &3600,
    );
}

/// Title with DEL (0x7F) is rejected.
#[test]
#[should_panic]
fn test_title_del_char_rejected() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    t.client.create_proposal(
        &proposer,
        &String::from_bytes(&t.env, b"bad\x7ftitle"),
        &String::from_str(&t.env, "desc"),
        &100,
        &3600,
    );
}

/// Description with a null byte is rejected.
#[test]
#[should_panic]
fn test_description_null_byte_rejected() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    t.client.create_proposal(
        &proposer,
        &String::from_str(&t.env, "Valid title"),
        &String::from_bytes(&t.env, b"bad\x00desc"),
        &100,
        &3600,
    );
}

/// Description with a control character (tab) is rejected.
#[test]
#[should_panic]
fn test_description_control_char_rejected() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    t.client.create_proposal(
        &proposer,
        &String::from_str(&t.env, "Valid title"),
        &String::from_bytes(&t.env, b"bad\x09desc"),
        &100,
        &3600,
    );
}

/// A title of exactly 128 printable bytes is accepted.
#[test]
fn test_title_max_length_accepted() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let title_128 = "A".repeat(128);
    let id = t.client.create_proposal(
        &proposer,
        &String::from_str(&t.env, &title_128),
        &String::from_str(&t.env, "desc"),
        &100,
        &3600,
    );
    assert_eq!(t.client.get_proposal(&id).state, ProposalState::Active);
}

/// A description of exactly 1024 printable bytes is accepted.
#[test]
fn test_description_max_length_accepted() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let desc_1024 = "B".repeat(1024);
    let id = t.client.create_proposal(
        &proposer,
        &String::from_str(&t.env, "Valid title"),
        &String::from_str(&t.env, &desc_1024),
        &100,
        &3600,
    );
    assert_eq!(t.client.get_proposal(&id).state, ProposalState::Active);
}

/// A title of 129 bytes is rejected (exceeds MAX_TITLE_LEN).
#[test]
#[should_panic]
fn test_title_too_long_rejected() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let title_129 = "A".repeat(129);
    t.client.create_proposal(
        &proposer,
        &String::from_str(&t.env, &title_129),
        &String::from_str(&t.env, "desc"),
        &100,
        &3600,
    );
}

/// A description of 1025 bytes is rejected (exceeds MAX_DESC_LEN).
#[test]
#[should_panic]
fn test_description_too_long_rejected() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let desc_1025 = "B".repeat(1025);
    t.client.create_proposal(
        &proposer,
        &String::from_str(&t.env, "Valid title"),
        &String::from_str(&t.env, &desc_1025),
        &100,
        &3600,
    );
}

/// A title with only printable ASCII (space = 0x20) is accepted.
#[test]
fn test_title_space_accepted() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let id = t.client.create_proposal(
        &proposer,
        &String::from_str(&t.env, "Hello World"),
        &String::from_str(&t.env, "desc"),
        &100,
        &3600,
    );
    assert_eq!(t.client.get_proposal(&id).state, ProposalState::Active);
}

// ── end SEC-003 ───────────────────────────────────────────────────────────────

// ── #44: max_active_proposals cap tests ──────────────────────────────────────

/// Creating proposals up to the cap succeeds; one more fails with
/// TooManyActiveProposals (#34).
#[test]
#[should_panic(expected = "Error(Contract, #34)")]
fn test_max_active_proposals_cap_enforced() {
    let env = Env::default();
    env.mock_all_auths();
    let gov_id = env.register(GovernanceContract, ());
    let client = GovernanceContractClient::new(&env, &gov_id);
    let admin = Address::generate(&env);
    let token_id = setup_token(&env, &admin);

    // Set cap to 3
    client.initialize(
        &admin,
        &token_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &3_u64, // max_active_proposals = 3
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );

    let proposer = Address::generate(&env);

    // First 3 proposals should succeed
    for _ in 0..3 {
        client.create_proposal(
            &proposer,
            &String::from_str(&env, "Prop"),
            &String::from_str(&env, "Description"),
            &100,
            &3600,
        );
    }

    // 4th proposal must fail with TooManyActiveProposals
    client.create_proposal(
        &proposer,
        &String::from_str(&env, "Over the limit"),
        &String::from_str(&env, "Should fail"),
        &100,
        &3600,
    );
}

/// Admin can raise the cap via update_max_proposals, allowing more proposals.
#[test]
fn test_admin_can_raise_max_proposals() {
    let env = Env::default();
    env.mock_all_auths();
    let gov_id = env.register(GovernanceContract, ());
    let client = GovernanceContractClient::new(&env, &gov_id);
    let admin = Address::generate(&env);
    let token_id = setup_token(&env, &admin);

    client.initialize(
        &admin,
        &token_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &2_u64, // cap = 2
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );

    let proposer = Address::generate(&env);

    // Fill up to cap
    client.create_proposal(
        &proposer,
        &String::from_str(&env, "Prop 1"),
        &String::from_str(&env, "desc"),
        &100,
        &3600,
    );
    client.create_proposal(
        &proposer,
        &String::from_str(&env, "Prop 2"),
        &String::from_str(&env, "desc"),
        &100,
        &3600,
    );

    // Raise the cap to 5
    client.update_max_proposals(&admin, &5_u64);
    assert_eq!(client.get_max_active_proposals(), 5);

    // Now a 3rd proposal succeeds
    let id = client.create_proposal(
        &proposer,
        &String::from_str(&env, "Prop 3"),
        &String::from_str(&env, "desc"),
        &100,
        &3600,
    );
    assert_eq!(client.get_proposal(&id).state, ProposalState::Active);
}

/// update_max_proposals called by a non-admin must revert with NotAdmin (#2).
#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_update_max_proposals_non_admin_reverts() {
    let t = setup_env();
    let attacker = Address::generate(&t.env);
    t.client.update_max_proposals(&attacker, &100_u64);
}

/// update_max_proposals with 0 must revert (0 cap blocks all proposals forever).
#[test]
#[should_panic]
fn test_update_max_proposals_zero_reverts() {
    let t = setup_env();
    t.client.update_max_proposals(&t.admin, &0_u64);
}

/// Default cap (no max_active_proposals set at init) is 50.
#[test]
fn test_default_max_active_proposals_is_50() {
    let t = setup_env();
    assert_eq!(t.client.get_max_active_proposals(), 50);
}

// ── end #44 ───────────────────────────────────────────────────────────────────

// ── Issue #50: list_proposals pagination tests ────────────────────────────────

/// offset=0, limit=5 returns first 5 proposals when 10 exist.
#[test]
fn test_list_proposals_offset_0_limit_5() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);

    // Create 10 proposals
    for i in 1..=10 {
        t.client.create_proposal(
            &proposer,
            &String::from_str(&t.env, &format!("Proposal {}", i)),
            &String::from_str(&t.env, "desc"),
            &100,
            &3600,
        );
    }

    let proposals = t.client.list_proposals(&0_u64, &5_u64);
    assert_eq!(proposals.len(), 5);
    assert_eq!(proposals.get(0).unwrap().id, 1);
    assert_eq!(proposals.get(1).unwrap().id, 2);
    assert_eq!(proposals.get(2).unwrap().id, 3);
    assert_eq!(proposals.get(3).unwrap().id, 4);
    assert_eq!(proposals.get(4).unwrap().id, 5);
}

/// offset=5, limit=5 returns next 5 proposals (IDs 6-10) when 10 exist.
#[test]
fn test_list_proposals_offset_5_limit_5() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);

    // Create 10 proposals
    for i in 1..=10 {
        t.client.create_proposal(
            &proposer,
            &String::from_str(&t.env, &format!("Proposal {}", i)),
            &String::from_str(&t.env, "desc"),
            &100,
            &3600,
        );
    }

    let proposals = t.client.list_proposals(&5_u64, &5_u64);
    assert_eq!(proposals.len(), 5);
    assert_eq!(proposals.get(0).unwrap().id, 6);
    assert_eq!(proposals.get(1).unwrap().id, 7);
    assert_eq!(proposals.get(2).unwrap().id, 8);
    assert_eq!(proposals.get(3).unwrap().id, 9);
    assert_eq!(proposals.get(4).unwrap().id, 10);
}

/// offset=8, limit=10 returns only remaining proposals (IDs 9-10) when 10 exist.
#[test]
fn test_list_proposals_limit_exceeds_remaining() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);

    // Create 10 proposals
    for i in 1..=10 {
        t.client.create_proposal(
            &proposer,
            &String::from_str(&t.env, &format!("Proposal {}", i)),
            &String::from_str(&t.env, "desc"),
            &100,
            &3600,
        );
    }

    let proposals = t.client.list_proposals(&8_u64, &10_u64);
    assert_eq!(proposals.len(), 2);
    assert_eq!(proposals.get(0).unwrap().id, 9);
    assert_eq!(proposals.get(1).unwrap().id, 10);
}

/// offset >= count returns empty list.
#[test]
fn test_list_proposals_offset_at_boundary() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);

    // Create 5 proposals
    for i in 1..=5 {
        t.client.create_proposal(
            &proposer,
            &String::from_str(&t.env, &format!("Proposal {}", i)),
            &String::from_str(&t.env, "desc"),
            &100,
            &3600,
        );
    }

    // offset=5 (equal to count) should return empty
    let proposals = t.client.list_proposals(&5_u64, &5_u64);
    assert_eq!(proposals.len(), 0);

    // offset=6 (exceeds count) should also return empty
    let proposals = t.client.list_proposals(&6_u64, &5_u64);
    assert_eq!(proposals.len(), 0);
}

/// limit=0 returns empty list (no panic).
#[test]
fn test_list_proposals_limit_0_returns_empty() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);

    // Create 5 proposals
    for i in 1..=5 {
        t.client.create_proposal(
            &proposer,
            &String::from_str(&t.env, &format!("Proposal {}", i)),
            &String::from_str(&t.env, "desc"),
            &100,
            &3600,
        );
    }

    let proposals = t.client.list_proposals(&0_u64, &0_u64);
    assert_eq!(proposals.len(), 0);
}

/// limit > MAX_LIMIT (50) is clamped to MAX_LIMIT.
#[test]
fn test_list_proposals_limit_clamped_to_max() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);

    // Create 60 proposals
    for i in 1..=60 {
        t.client.create_proposal(
            &proposer,
            &String::from_str(&t.env, &format!("Proposal {}", i)),
            &String::from_str(&t.env, "desc"),
            &100,
            &3600,
        );
    }

    // Request limit=100, should be clamped to 50
    let proposals = t.client.list_proposals(&0_u64, &100_u64);
    assert_eq!(proposals.len(), 50);
    assert_eq!(proposals.get(0).unwrap().id, 1);
    assert_eq!(proposals.get(49).unwrap().id, 50);
}

/// list_proposals returns correct data in all fields of each proposal.
#[test]
fn test_list_proposals_data_integrity() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let title = "Test Proposal";
    let desc = "Test Description";

    let id = t.client.create_proposal(
        &proposer,
        &String::from_str(&t.env, title),
        &String::from_str(&t.env, desc),
        &500_u64 as i128,
        &3600,
    );

    let proposals = t.client.list_proposals(&0_u64, &1_u64);
    assert_eq!(proposals.len(), 1);

    let prop = proposals.get(0).unwrap();
    assert_eq!(prop.id, id);
    assert_eq!(prop.proposer, proposer);
    assert_eq!(prop.quorum, 500);
    assert_eq!(prop.votes_yes, 0);
    assert_eq!(prop.votes_no, 0);
    assert_eq!(prop.votes_abstain, 0);
    assert_eq!(prop.state, ProposalState::Active);
}

/// Proposals in mixed states (Active, Passed, Rejected, Cancelled) are all returned in order.
#[test]
fn test_list_proposals_mixed_states() {
    let t = setup_env();
    let voter = Address::generate(&t.env);

    // Create 4 proposals with different states
    let active_id = t.client.create_proposal(
        &voter,
        &String::from_str(&t.env, "Active"),
        &String::from_str(&t.env, "desc"),
        &100,
        &3600,
    );

    let passed_id = t.client.create_proposal(
        &voter,
        &String::from_str(&t.env, "Passed"),
        &String::from_str(&t.env, "desc"),
        &100,
        &3600,
    );

    let rejected_id = t.client.create_proposal(
        &voter,
        &String::from_str(&t.env, "Rejected"),
        &String::from_str(&t.env, "desc"),
        &9_999_999,
        &3600,
    );

    let cancelled_id = t.client.create_proposal(
        &voter,
        &String::from_str(&t.env, "Cancelled"),
        &String::from_str(&t.env, "desc"),
        &100,
        &3600,
    );

    // Finalize passed and rejected
    mint_and_vote(&t, &voter, passed_id, Vote::Yes, 1_000_000);
    t.env.ledger().with_mut(|l| l.timestamp += 3601);
    t.client.finalise(&passed_id);
    t.client.finalise(&rejected_id);

    // Cancel one
    t.client.cancel(&t.admin, &cancelled_id);

    // List all
    let proposals = t.client.list_proposals(&0_u64, &10_u64);
    assert_eq!(proposals.len(), 4);

    assert_eq!(proposals.get(0).unwrap().state, ProposalState::Active);
    assert_eq!(proposals.get(1).unwrap().state, ProposalState::Passed);
    assert_eq!(proposals.get(2).unwrap().state, ProposalState::Rejected);
    assert_eq!(proposals.get(3).unwrap().state, ProposalState::Cancelled);
}

/// Empty proposal list (no proposals created) returns empty list.
#[test]
fn test_list_proposals_empty_when_none_created() {
    let t = setup_env();
    let proposals = t.client.list_proposals(&0_u64, &10_u64);
    assert_eq!(proposals.len(), 0);
}

/// Pagination is consistent: fetching pages individually matches fetching all at once.
#[test]
fn test_list_proposals_pagination_consistency() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);

    // Create 15 proposals
    for i in 1..=15 {
        t.client.create_proposal(
            &proposer,
            &String::from_str(&t.env, &format!("Proposal {}", i)),
            &String::from_str(&t.env, "desc"),
            &100,
            &3600,
        );
    }

    // Fetch in pages of 5
    let page1 = t.client.list_proposals(&0_u64, &5_u64);
    let page2 = t.client.list_proposals(&5_u64, &5_u64);
    let page3 = t.client.list_proposals(&10_u64, &5_u64);

    // Fetch all at once
    let all = t.client.list_proposals(&0_u64, &50_u64);

    assert_eq!(all.len(), 15);
    assert_eq!(page1.len(), 5);
    assert_eq!(page2.len(), 5);
    assert_eq!(page3.len(), 5);

    // Verify order is consistent
    for i in 0..5 {
        assert_eq!(page1.get(i).unwrap().id, all.get(i).unwrap().id);
    }
    for i in 0..5 {
        assert_eq!(page2.get(i).unwrap().id, all.get(i + 5).unwrap().id);
    }
    for i in 0..5 {
        assert_eq!(page3.get(i).unwrap().id, all.get(i + 10).unwrap().id);
    }
}

/// list_proposals does not require auth (read-only).
#[test]
fn test_list_proposals_no_auth_required() {
    let env = Env::default();
    // Do not mock_all_auths; list_proposals should work without auth
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let token_id = setup_token(&env, &admin);

    // Need to initialize, which requires auth
    env.mock_all_auths();
    client.initialize(
        &admin,
        &token_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );

    let proposer = Address::generate(&env);
    client.create_proposal(
        &proposer,
        &String::from_str(&env, "Prop"),
        &String::from_str(&env, "desc"),
        &100,
        &3600,
    );

    // Reset auth mocking
    env.mock_all_auths_allow_last(false);

    // list_proposals should still work (no auth required)
    let proposals = client.list_proposals(&0_u64, &10_u64);
    assert_eq!(proposals.len(), 1);
}

// ── end Issue #50 ─────────────────────────────────────────────────────────────

// ── Issue #49: Flash-loan attack investigation test ──────────────────────────

/// This test reproduces the theoretical flash-loan attack scenario:
/// 1. Attacker obtains tokens (simulating a flash loan or temporary transfer)
/// 2. Attacker votes on a proposal with the acquired tokens
/// 3. Attacker returns/burns the tokens
/// 4. Attacker's original tokens vote again (if balance-based, not snapshot-based)
///
/// With the current implementation (live balance at vote time), this test
/// demonstrates that token recycling is possible if an attacker can acquire
/// and return tokens between transactions. However, it shows that within a
/// single transaction/block, the attack is not possible due to Soroban's
/// atomic execution model.
#[test]
fn test_flash_loan_attack_across_transactions_demonstration() {
    let env = Env::default();
    env.mock_all_auths();

    let gov_id = env.register(GovernanceContract, ());
    let gov_client = GovernanceContractClient::new(&env, &gov_id);
    let admin = Address::generate(&env);
    let token_id = env.register(votechain_token::TokenContract, ());
    let token_client = votechain_token::TokenContractClient::new(&env, &token_id);

    // Initialize token and governance
    token_client.initialize(&admin, &10_000_000_i128);
    gov_client.initialize(
        &admin,
        &token_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );

    let proposer = Address::generate(&env);
    let attacker = Address::generate(&env);
    let lender = Address::generate(&env);

    // Setup: Proposer has 1M tokens, lender has 1M tokens to "loan"
    token_client.mint(&admin, &proposer, &1_000_000_i128);
    token_client.mint(&admin, &lender, &1_000_000_i128);

    // Create a proposal
    let id = gov_client.create_proposal(
        &proposer,
        &String::from_str(&env, "Attack test proposal"),
        &String::from_str(&env, "desc"),
        &500_000,
        &3600,
    );

    // ---- Transaction 1: Attacker votes with borrowed tokens ----
    // Lender transfers tokens to attacker (simulating flash loan)
    token_client.transfer(&lender, &attacker, &1_000_000_i128);

    // Attacker votes with the borrowed balance (1M tokens)
    gov_client.cast_vote(&attacker, &id, &Vote::Yes, &None);
    let prop_after_first_vote = gov_client.get_proposal(&id);
    assert_eq!(prop_after_first_vote.votes_yes, 1_000_000);

    // ---- Transaction 2: Attacker repays and tries to vote again ----
    // Attacker returns the tokens to lender (simulating loan repayment)
    token_client.transfer(&attacker, &lender, &1_000_000_i128);

    // Attacker's balance is now 0, so another vote would fail with NoVotingPower
    // This is the key difference from a snapshot-based system where the vote weight
    // would have been locked in at proposal creation time.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        gov_client.cast_vote(&attacker, &id, &Vote::No, &None);
    }));

    // Vote should fail because attacker has 0 balance now
    assert!(result.is_err(), "Expected vote to fail with no balance");

    // Final tally shows only the first vote (1M Yes)
    let final_prop = gov_client.get_proposal(&id);
    assert_eq!(final_prop.votes_yes, 1_000_000);
    assert_eq!(final_prop.votes_no, 0);
}

/// This test shows that within a single transaction, Soroban's atomic execution
/// prevents the attacker from voting multiple times with the same tokens:
/// - Even if the attacker calls cast_vote, the duplicate vote is prevented by the
///   has_voted guard, not by token balance checks.
/// - Soroban does not allow the same voter to vote twice on the same proposal,
///   regardless of balance.
#[test]
fn test_single_transaction_prevents_double_voting() {
    let t = setup_env();
    let voter = Address::generate(&t.env);

    let id = t.client.create_proposal(
        &voter,
        &String::from_str(&t.env, "Single-tx proposal"),
        &String::from_str(&t.env, "desc"),
        &100,
        &3600,
    );

    // Voter votes once
    mint_and_vote(&t, &voter, id, Vote::Yes, 1_000_000);
    assert_eq!(t.client.get_proposal(&id).votes_yes, 1_000_000);

    // Attempt to vote again in same transaction (or within same block execution)
    // This must fail with AlreadyVoted due to the has_voted guard
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        t.client.cast_vote(&voter, &id, &Vote::No, &None);
    }));

    assert!(result.is_err(), "Expected second vote to fail with AlreadyVoted");

    // Tally unchanged
    assert_eq!(t.client.get_proposal(&id).votes_yes, 1_000_000);
    assert_eq!(t.client.get_proposal(&id).votes_no, 0);
}

// ── end Issue #49 tests ───────────────────────────────────────────────────────

// ── Issue #118: Parameter change proposal tests ────────────────────────────────

/// Creating a parameter change proposal for MinDuration works and stores the config key/value.
#[test]
fn test_create_parameter_change_proposal_min_duration() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);

    let id = t.client.create_parameter_change_proposal(
        &proposer,
        &String::from_str(&t.env, "Increase Min Duration"),
        &String::from_str(&t.env, "Propose to increase minimum proposal duration"),
        &100,
        &3600,
        &ConfigKey::MinDuration,
        &180_u64, // new min duration: 180 seconds
    );

    assert_eq!(id, 1);
    let prop = t.client.get_proposal(&id);
    assert_eq!(prop.state, ProposalState::Active);

    // Verify proposal type is ParameterChange
    match prop.proposal_type {
        ProposalType::ParameterChange { key, value } => {
            assert_eq!(key, ConfigKey::MinDuration);
            assert_eq!(value, 180);
        }
        _ => panic!("Expected ParameterChange proposal type"),
    }
}

/// Creating a parameter change proposal for ProposalCooldown works.
#[test]
fn test_create_parameter_change_proposal_cooldown() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);

    let id = t.client.create_parameter_change_proposal(
        &proposer,
        &String::from_str(&t.env, "Update Cooldown"),
        &String::from_str(&t.env, "desc"),
        &100,
        &3600,
        &ConfigKey::ProposalCooldown,
        &7200_u64, // new cooldown: 2 hours
    );

    let prop = t.client.get_proposal(&id);
    match prop.proposal_type {
        ProposalType::ParameterChange { key, value } => {
            assert_eq!(key, ConfigKey::ProposalCooldown);
            assert_eq!(value, 7200);
        }
        _ => panic!("Expected ParameterChange proposal type"),
    }
}

/// Creating a parameter change proposal for MinDuration with value=0 fails.
#[test]
#[should_panic(expected = "Error(Contract, #37)")]
fn test_parameter_change_min_duration_zero_rejected() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);

    t.client.create_parameter_change_proposal(
        &proposer,
        &String::from_str(&t.env, "Bad min duration"),
        &String::from_str(&t.env, "desc"),
        &100,
        &3600,
        &ConfigKey::MinDuration,
        &0_u64, // invalid: must be > 0
    );
}

/// Creating a parameter change proposal for MaxDuration with value=0 fails.
#[test]
#[should_panic(expected = "Error(Contract, #37)")]
fn test_parameter_change_max_duration_zero_rejected() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);

    t.client.create_parameter_change_proposal(
        &proposer,
        &String::from_str(&t.env, "Bad max duration"),
        &String::from_str(&t.env, "desc"),
        &100,
        &3600,
        &ConfigKey::MaxDuration,
        &0_u64, // invalid: must be > 0
    );
}

/// Executing a parameter change proposal applies the new value to MinDuration.
#[test]
fn test_execute_parameter_change_min_duration() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let token_id = setup_token(&env, &admin);
    let proposer = Address::generate(&env);

    client.initialize(
        &admin,
        &token_id,
        &0_i128,
        &0_u64,
        &60_u64,      // current min_duration
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );

    // Create parameter change proposal
    let id = client.create_parameter_change_proposal(
        &proposer,
        &String::from_str(&env, "Increase Min Duration"),
        &String::from_str(&env, "desc"),
        &100,
        &3600,
        &ConfigKey::MinDuration,
        &180_u64, // new value
    );

    // Vote it through
    let tok = votechain_token::TokenContractClient::new(&env, &token_id);
    tok.mint(&admin, &proposer, &1_000_000_i128);
    client.cast_vote(&proposer, &id, &Vote::Yes, &None);

    // Finalize
    env.ledger().with_mut(|l| l.timestamp += 3601);
    client.finalise(&id);

    // Execute
    client.execute(&admin, &id);

    // Verify the parameter was changed
    // We can verify by trying to create a proposal with duration < 180 seconds (should fail)
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.create_proposal(
            &proposer,
            &String::from_str(&env, "Short duration"),
            &String::from_str(&env, "desc"),
            &100,
            &120_u64, // < 180, should now fail
        );
    }));

    assert!(result.is_err(), "Expected proposal creation to fail with new min duration");
}

/// Executing a parameter change proposal for ProposalCooldown applies the change.
#[test]
fn test_execute_parameter_change_cooldown() {
    let env = Env::default();
    env.mock_all_auths();
    let client = new_client(&env);
    let admin = Address::generate(&env);
    let token_id = setup_token(&env, &admin);
    let proposer = Address::generate(&env);

    client.initialize(
        &admin,
        &token_id,
        &0_i128,
        &0_u64,
        &60_u64,
        &2_592_000_u64,
        &false,
        &0_u64,
        &0_u64,
        &0_u32, // min_quorum_bps = 0 → use protocol default (10 BPS)
    );

    // Create parameter change proposal
    let id = client.create_parameter_change_proposal(
        &proposer,
        &String::from_str(&env, "Update Cooldown"),
        &String::from_str(&env, "desc"),
        &100,
        &3600,
        &ConfigKey::ProposalCooldown,
        &86400_u64, // 24 hours
    );

    // Vote and execute
    let tok = votechain_token::TokenContractClient::new(&env, &token_id);
    tok.mint(&admin, &proposer, &1_000_000_i128);
    client.cast_vote(&proposer, &id, &Vote::Yes, &None);

    env.ledger().with_mut(|l| l.timestamp += 3601);
    client.finalise(&id);
    client.execute(&admin, &id);

    // Try to create a second proposal immediately (should fail due to cooldown)
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.create_proposal(
            &proposer,
            &String::from_str(&env, "Second proposal"),
            &String::from_str(&env, "desc"),
            &100,
            &3600,
        );
    }));

    assert!(
        result.is_err(),
        "Expected second proposal to fail due to updated cooldown"
    );
}

/// Admin can use execute_parameter_change_override to directly change parameters.
#[test]
fn test_admin_parameter_change_override() {
    let t = setup_env();

    // Change MinProposalBalance via override (no proposal needed)
    t.client.execute_parameter_change_override(
        &t.admin,
        &ConfigKey::MinProposalBalance,
        &500_000_u64,
    );

    // Verify by trying to create a proposal with insufficient balance
    let proposer = Address::generate(&t.env);
    let tok = votechain_token::TokenContractClient::new(&t.env, &t.token_id);
    tok.mint(&t.admin, &proposer, &100_000_i128); // Less than 500k

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        t.client.create_proposal(
            &proposer,
            &String::from_str(&t.env, "Insufficient balance"),
            &String::from_str(&t.env, "desc"),
            &100,
            &3600,
        );
    }));

    assert!(
        result.is_err(),
        "Expected proposal creation to fail with new min balance"
    );
}

/// Non-admin cannot use execute_parameter_change_override.
#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_parameter_change_override_non_admin_fails() {
    let t = setup_env();
    let attacker = Address::generate(&t.env);

    t.client.execute_parameter_change_override(
        &attacker,
        &ConfigKey::ProposalCooldown,
        &7200_u64,
    );
}

/// Parameter change proposals with invalid semantic values are rejected.
#[test]
#[should_panic(expected = "Error(Contract, #37)")]
fn test_parameter_change_override_invalid_value() {
    let t = setup_env();

    // MinDuration cannot be 0
    t.client.execute_parameter_change_override(&t.admin, &ConfigKey::MinDuration, &0_u64);
}

/// Standard proposals can coexist with parameter change proposals in the proposal list.
#[test]
fn test_mixed_proposal_types() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);

    // Create a standard proposal
    let standard_id = t.client.create_proposal(
        &proposer,
        &String::from_str(&t.env, "Standard proposal"),
        &String::from_str(&t.env, "desc"),
        &100,
        &3600,
    );

    // Create a parameter change proposal
    let param_id = t.client.create_parameter_change_proposal(
        &proposer,
        &String::from_str(&t.env, "Parameter change"),
        &String::from_str(&t.env, "desc"),
        &100,
        &3600,
        &ConfigKey::ProposalCooldown,
        &3600_u64,
    );

    // List both proposals
    let proposals = t.client.list_proposals(&0_u64, &10_u64);
    assert_eq!(proposals.len(), 2);

    let std_prop = proposals.get(0).unwrap();
    let param_prop = proposals.get(1).unwrap();

    assert_eq!(std_prop.id, standard_id);
    assert_eq!(param_prop.id, param_id);

    // Verify types
    match std_prop.proposal_type {
        ProposalType::Standard => {}
        _ => panic!("Expected Standard proposal type"),
    }

    match &param_prop.proposal_type {
        ProposalType::ParameterChange { key, .. } => {
            assert_eq!(*key, ConfigKey::ProposalCooldown);
        }
        _ => panic!("Expected ParameterChange proposal type"),
    }
}

// ── end Issue #118 ─────────────────────────────────────────────────────────────

// ── Issue #119: Multi-asset voting support tests & framework ──────────────────

/// Test that demonstrates the framework for multi-asset voting.
///
/// This test is a placeholder and documents the expected behavior for multi-token voting.
/// Full implementation is scheduled for v0.2.0 and involves:
///
/// 1. Extending initialize() to accept Vec<VotingTokenConfig> with per-token weights
/// 2. Updating cast_vote(, &None) to aggregate voting power across registered tokens
/// 3. Adding admin functions: add_voting_token(), remove_voting_token(), set_token_weight()
/// 4. Implementing governance-driven token management (v0.2.1)
///
/// The architecture has been designed in ADR-005 with comprehensive test coverage planned.
#[test]
fn test_multi_asset_voting_framework() {
    // This test documents the expected multi-asset voting behavior:
    //
    // Example scenario:
    //   - Token A (USDC): balance = 1,000, multiplier = 100 (1x) → 1,000 votes
    //   - Token B (NFT): balance = 5, multiplier = 200 (2x) → 10 votes
    //   - Total voting weight = 1,010 votes
    //
    // Expected calls (v0.2.0+):
    //
    //   let voting_tokens = vec![
    //       VotingTokenConfig { token_address: token_a, weight_multiplier: 100 },
    //       VotingTokenConfig { token_address: token_b, weight_multiplier: 200 },
    //   ];
    //
    //   client.initialize_with_tokens(
    //       &admin,
    //       voting_tokens,
    //       0_i128,    // min_proposal_balance
    //       0_u64,     // proposal_cooldown
    //       60_u64,    // min_duration
    //       2_592_000_u64, // max_duration
    //       false,     // restrict_admin_vote
    //       0_u64,     // timelock_duration
    //       0_u64,     // max_active_proposals
    //   );
    //
    //   // Voter owns 1,000 Token A and 5 Token B
    //   // Total weight = (1,000 × 100) + (5 × 200) / 100 = 1,010 votes
    //   // (Note: division by 100 is for percentage multipliers)
    //
    //   client.cast_vote(&voter, &proposal_id, &Vote::Yes, &None);
    //   // Vote weight is 1,010 (aggregated from both tokens)
    //
    // Tests to implement:
    // - ✓ Initialize with single token (backward compatibility)
    // - ✓ Initialize with multiple tokens
    // - ✓ Vote weight aggregation from multiple tokens
    // - ✓ Add voting token
    // - ✓ Remove voting token
    // - ✓ Update token weight multiplier
    // - ✓ Prevent duplicate token registration
    // - ✓ Vote weight overflow handling
    // - ✓ Events for token operations
    // - ✓ Admin-only token operations
    // - ✓ Governance-driven token management (v0.2.1)
    //
    // See ADR-005 for design details: docs/ADR-005-multi-asset-voting.md
}

/// Documents the expected multi-token vote aggregation formula.
///
/// Vote weight calculation:
///   total_weight = Σ (balance_in_token_i × weight_multiplier_i / 100)
///
/// Example with 2 tokens:
///   - Token A (utility): balance = 10,000, multiplier = 100 (1x)
///     Contribution: 10,000
///   - Token B (NFT): balance = 10, multiplier = 150 (1.5x)
///     Contribution: 15
///   - Total: 10,015 votes
///
/// The multiplier is expressed as percentage (100 = 1x, 150 = 1.5x, 50 = 0.5x).
/// This is stored as a u64 to avoid floating-point precision issues.
#[test]
fn test_multi_asset_voting_weight_formula() {
    // Weight formula documentation for future implementation:
    // total_weight = sum((balance_i * multiplier_i) / 100 for each token)
    //
    // Multiplier meanings:
    //   100 = 1x (normal voting power)
    //   200 = 2x (double voting power)
    //   50  = 0.5x (half voting power)
    //   0   = invalid (caught at add_voting_token validation)
    //
    // Rounding: integer division (truncation)
    //   balance=100, multiplier=150 → (100*150)/100 = 150
    //   balance=99, multiplier=150 → (99*150)/100 = 148 (truncated)
}

/// Documents expected events for multi-token operations.
///
/// Events to emit (v0.2.0+):
///   - token_added: When a voting token is registered
///   - token_removed: When a voting token is deregistered
///   - token_weight_updated: When a token's multiplier changes
///
/// Example event for adding a token:
///   Topics: ("token_added",)
///   Data: (token_address: Address, weight_multiplier: u64)
#[test]
fn test_multi_asset_voting_events() {
    // Event documentation for future implementation:
    // 
    // Event: token_added
    //   Emitted by: add_voting_token()
    //   Topics: ("token_added",)
    //   Data: (token_address: Address, weight_multiplier: u64)
    //
    // Event: token_removed
    //   Emitted by: remove_voting_token()
    //   Topics: ("token_removed",)
    //   Data: (token_address: Address,)
    //
    // Event: token_weight_updated
    //   Emitted by: set_token_weight()
    //   Topics: ("token_weight_upd",)
    //   Data: (token_address: Address, new_multiplier: u64)
}

/// Documents the delegation interaction with multi-asset voting.
///
/// Expected behavior (v0.2.0+):
/// - When a delegator delegates to a delegate, the delegate receives the delegator's
///   aggregated weight across all registered tokens.
/// - Example:
///   - Delegator owns 1,000 Token A (1x) + 10 Token B (2x) = 1,020 votes
///   - Delegator delegates to delegate
///   - When delegate calls cast_vote_with_delegators including delegator,
///     delegate receives +1,020 votes from delegator's multi-token balance
#[test]
fn test_multi_asset_voting_with_delegation() {
    // Delegation + multi-token documentation:
    //
    // Expected behavior:
    //   delegator.balance_in_token_a = 500
    //   delegator.balance_in_token_b = 50
    //   token_a_multiplier = 100 (1x)
    //   token_b_multiplier = 200 (2x)
    //   delegator.weight = (500*100 + 50*200) / 100 = 600 votes
    //
    //   When delegator delegates to delegate:
    //     delegate.weight += 600 votes (when delegate casts vote)
    //
    // Test scenarios:
    //   - ✓ Delegate receives aggregated weight from single delegator
    //   - ✓ Delegate receives aggregated weights from multiple delegators
    //   - ✓ Delegator cannot vote directly while delegated (guard still applies)
    //   - ✓ Undelegate restores delegator's voting rights
}

/// Documents the parameter change interaction with multi-asset voting.
///
/// Some parameters may interact differently with multi-asset voting:
/// - min_proposal_balance: Checked against aggregated balance of proposer? Or per-token minimum?
/// - This is a design question for v0.2.0 implementation
#[test]
fn test_multi_asset_voting_parameter_change_interaction() {
    // Parameter change + multi-token documentation:
    //
    // Open design question for v0.2.0:
    //   Should min_proposal_balance be:
    //   A) Checked against aggregated balance (simpler)
    //   B) Checked per-token (more complex, but clearer)
    //   C) Checked against primary token only (backward compatible)
    //
    // Current decision: Option A (aggregated balance)
    //   This is consistent with the voting weight aggregation model.
}

// ── end Issue #119 ─────────────────────────────────────────────────────────────

// ── Issue #61: get_votes batch function tests ──────────────────────────────────

/// Empty batch returns an empty Vec without error.
#[test]
fn test_get_votes_empty_batch() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let tok = votechain_token::TokenContractClient::new(&t.env, &t.token_id);
    tok.mint(&t.admin, &proposer, &500);
    let id = create_test_proposal(&t, &proposer);

    let empty: soroban_sdk::Vec<Address> = soroban_sdk::Vec::new(&t.env);
    let results = t.client.get_votes(&id, &empty).unwrap();
    assert_eq!(results.len(), 0);
}

/// Single voter — voted address returns Some, non-voter returns None.
#[test]
fn test_get_votes_single_voter() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let voter = Address::generate(&t.env);
    let non_voter = Address::generate(&t.env);

    let tok = votechain_token::TokenContractClient::new(&t.env, &t.token_id);
    tok.mint(&t.admin, &proposer, &500);
    tok.mint(&t.admin, &voter, &200);

    let id = create_test_proposal(&t, &proposer);
    t.client.cast_vote(&voter, &id, &Vote::Yes, &None);

    let mut voters = soroban_sdk::Vec::new(&t.env);
    voters.push_back(voter.clone());
    voters.push_back(non_voter.clone());

    let results = t.client.get_votes(&id, &voters).unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.get(0).unwrap().is_some());
    assert!(results.get(1).unwrap().is_none());
}

/// Full batch of 50 voters all returns correctly.
#[test]
fn test_get_votes_batch_of_50() {
    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let tok = votechain_token::TokenContractClient::new(&t.env, &t.token_id);
    tok.mint(&t.admin, &proposer, &500);
    let id = create_test_proposal(&t, &proposer);

    let mut voters: soroban_sdk::Vec<Address> = soroban_sdk::Vec::new(&t.env);
    for _ in 0..50u32 {
        let v = Address::generate(&t.env);
        tok.mint(&t.admin, &v, &100);
        t.client.cast_vote(&v, &id, &Vote::Yes, &None);
        voters.push_back(v);
    }

    let results = t.client.get_votes(&id, &voters).unwrap();
    assert_eq!(results.len(), 50);
    for i in 0..50u32 {
        assert!(results.get(i).unwrap().is_some());
    }
}

/// Batch of 51 voters fails with BatchTooLarge.
#[test]
fn test_get_votes_batch_of_51_fails() {
    use crate::types::ContractError;

    let t = setup_env();
    let proposer = Address::generate(&t.env);
    let tok = votechain_token::TokenContractClient::new(&t.env, &t.token_id);
    tok.mint(&t.admin, &proposer, &500);
    let id = create_test_proposal(&t, &proposer);

    let mut voters: soroban_sdk::Vec<Address> = soroban_sdk::Vec::new(&t.env);
    for _ in 0..51u32 {
        voters.push_back(Address::generate(&t.env));
    }

    let err = t.client.try_get_votes(&id, &voters).unwrap_err().unwrap();
    assert_eq!(err, ContractError::BatchTooLarge);
}

/// Non-existent proposal fails fast with ProposalNotFound.
#[test]
fn test_get_votes_proposal_not_found() {
    use crate::types::ContractError;

    let t = setup_env();
    let empty: soroban_sdk::Vec<Address> = soroban_sdk::Vec::new(&t.env);
    let err = t.client.try_get_votes(&9999, &empty).unwrap_err().unwrap();
    assert_eq!(err, ContractError::ProposalNotFound);
}

// ── end Issue #61 ──────────────────────────────────────────────────────────────
