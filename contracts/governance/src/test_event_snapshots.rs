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

//! # Issue #55 / #299 – Snapshot tests for all contract event structures
//!
//! Each test exercises exactly one event type and asserts on its topics/data
//! shape.  The soroban test framework captures the full ledger state after
//! each test run and stores it in
//! `test_snapshots/test/{test_name}.1.json`.  CI fails if any snapshot
//! diverges from the committed file without an explicit update.
//!
//! See CONTRIBUTING.md → **Updating test snapshots** for the update workflow.
//!
//! ## Events covered
//!
//! | Event                    | Symbol       | Test                                          |
//! |--------------------------|--------------|-----------------------------------------------|
//! | `contract_initialized`   | `"init"`     | `test_event_snapshot_contract_initialized`    |
//! | `proposal_created`       | `"created"`  | `test_event_snapshot_proposal_created`        |
//! | `vote_cast`              | `"vote"`     | `test_event_snapshot_vote_cast`               |
//! | `proposal_finalised`     | `"final"`    | `test_event_snapshot_proposal_finalised`      |
//! | `proposal_executed`      | `"executed"` | `test_event_snapshot_proposal_executed`       |
//! | `proposal_cancelled`     | `"cancelled"`| `test_event_snapshot_proposal_cancelled`      |
//! | `quorum_updated`         | `"qupdate"`  | `test_event_snapshot_quorum_updated`          |
//! | `admin_transferred`      | `"admxfer"`  | `test_event_snapshot_admin_transferred`       |
//! | `contract_paused`        | `"paused"`   | `test_event_snapshot_contract_paused`         |
//! | `contract_unpaused`      | `"unpaused"` | `test_event_snapshot_contract_unpaused`       |
//! | `admin_transfer_proposed`| `"admprop"`  | `test_event_snapshot_admin_transfer_proposed` |

#![cfg(test)]

use super::*;
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events, Ledger},
    Address, Env, IntoVal, String, TryFromVal,
};

// ─────────────────────────────────────────────────────────────────────────────
// Shared test helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Deploy a fresh token contract, mint `10_000_000` to `admin`, return its id.
fn snap_token(env: &Env, admin: &Address) -> Address {
    let id = env.register(votechain_token::TokenContract, ());
    votechain_token::TokenContractClient::new(env, &id).initialize(admin, &10_000_000);
    id
}

/// Deploy a governance contract initialised with default config.
fn snap_gov(env: &Env, admin: &Address, token: &Address) -> GovernanceContractClient<'static> {
    let id = env.register(GovernanceContract, ());
    let c = GovernanceContractClient::new(env, &id);
    c.initialize(admin, token, &0_i128, &0_u64, &60_u64, &2_592_000_u64, &false, &0_u64, &0_u64, &0_u32);
    c
}

/// Create an active proposal with `voter` as proposer (voter must have tokens).
fn snap_proposal(env: &Env, c: &GovernanceContractClient, voter: &Address) -> u64 {
    c.create_proposal(
        voter,
        &String::from_str(env, "Event snapshot proposal"),
        &String::from_str(env, "Verifying event structure"),
        &100_i128,
        &3600_u64,
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Event snapshot tests
// ─────────────────────────────────────────────────────────────────────────────

/// `contract_initialized` — topics: `("init",)`, data: `admin: Address`
#[test]
fn test_event_snapshot_contract_initialized() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let token = snap_token(&env, &admin);
    let id = env.register(GovernanceContract, ());
    let c = GovernanceContractClient::new(&env, &id);
    c.initialize(&admin, &token, &0, &0, &60, &2_592_000, &false, &0, &0, &0_u32);

    let events = env.events().all();
    assert!(
        events.iter().any(|(_, topics, data)| {
            topics == (symbol_short!("init"),).into_val(&env)
                && Address::try_from_val(&env, &data).is_ok()
        }),
        "expected 'init' event with admin Address"
    );
}

/// `proposal_created` — topics: `("created", id: u64)`, data: `proposer: Address`
#[test]
fn test_event_snapshot_proposal_created() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let voter = Address::generate(&env);
    let token = snap_token(&env, &admin);
    votechain_token::TokenContractClient::new(&env, &token)
        .transfer(&admin, &voter, &500_000);
    let c = snap_gov(&env, &admin, &token);
    let pid = snap_proposal(&env, &c, &voter);

    let events = env.events().all();
    assert!(
        events.iter().any(|(_, topics, data)| {
            topics == (symbol_short!("created"), pid).into_val(&env)
                && Address::try_from_val(&env, &data).is_ok()
        }),
        "expected 'created' event with (id, proposer)"
    );
}

/// `vote_cast` — topics: `("vote", id)`,
/// data: `(voter: Address, vote: Vote, weight: i128, balance_snapshot: i128)`
#[test]
fn test_event_snapshot_vote_cast() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let voter = Address::generate(&env);
    let token = snap_token(&env, &admin);
    votechain_token::TokenContractClient::new(&env, &token)
        .transfer(&admin, &voter, &500_000);
    let c = snap_gov(&env, &admin, &token);
    let pid = snap_proposal(&env, &c, &voter);
    c.cast_vote(&voter, &pid, &Vote::Yes, &None);

    let events = env.events().all();
    assert!(
        events.iter().any(|(_, topics, _)| {
            topics == (symbol_short!("vote"), pid).into_val(&env)
        }),
        "expected 'vote' event with proposal id"
    );
}

/// `proposal_finalised` — topics: `("final", id)`,
/// data: `(state: ProposalState, execute_after: u64)`
#[test]
fn test_event_snapshot_proposal_finalised() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let voter = Address::generate(&env);
    let token = snap_token(&env, &admin);
    votechain_token::TokenContractClient::new(&env, &token)
        .transfer(&admin, &voter, &500_000);
    let c = snap_gov(&env, &admin, &token);
    let pid = snap_proposal(&env, &c, &voter);
    c.cast_vote(&voter, &pid, &Vote::Yes, &None);
    env.ledger().with_mut(|l| l.timestamp += 3601);
    c.finalise(&pid);

    let events = env.events().all();
    assert!(
        events.iter().any(|(_, topics, _)| {
            topics == (symbol_short!("final"), pid).into_val(&env)
        }),
        "expected 'final' event with proposal id"
    );
}

/// `proposal_executed` — topics: `("executed", id)`, data: `()`
#[test]
fn test_event_snapshot_proposal_executed() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let voter = Address::generate(&env);
    let token = snap_token(&env, &admin);
    votechain_token::TokenContractClient::new(&env, &token)
        .transfer(&admin, &voter, &500_000);
    let c = snap_gov(&env, &admin, &token);
    let pid = snap_proposal(&env, &c, &voter);
    c.cast_vote(&voter, &pid, &Vote::Yes, &None);
    env.ledger().with_mut(|l| l.timestamp += 3601);
    c.finalise(&pid);
    c.execute(&admin, &pid);

    let events = env.events().all();
    assert!(
        events.iter().any(|(_, topics, _)| {
            topics == (symbol_short!("executed"), pid).into_val(&env)
        }),
        "expected 'executed' event with proposal id"
    );
}

/// `proposal_cancelled` — topics: `("cancelled", id)`, data: `()`
#[test]
fn test_event_snapshot_proposal_cancelled() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let voter = Address::generate(&env);
    let token = snap_token(&env, &admin);
    votechain_token::TokenContractClient::new(&env, &token)
        .transfer(&admin, &voter, &500_000);
    let c = snap_gov(&env, &admin, &token);
    let pid = snap_proposal(&env, &c, &voter);
    c.cancel(&admin, &pid);

    let events = env.events().all();
    assert!(
        events.iter().any(|(_, topics, _)| {
            topics == (symbol_short!("cancelled"), pid).into_val(&env)
        }),
        "expected 'cancelled' event with proposal id"
    );
}

/// `quorum_updated` — topics: `("qupdate", id)`, data: `new_quorum: i128`
#[test]
fn test_event_snapshot_quorum_updated() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let voter = Address::generate(&env);
    let token = snap_token(&env, &admin);
    votechain_token::TokenContractClient::new(&env, &token)
        .transfer(&admin, &voter, &500_000);
    let c = snap_gov(&env, &admin, &token);
    let pid = snap_proposal(&env, &c, &voter);
    c.update_quorum(&admin, &pid, &200_i128);

    let events = env.events().all();
    assert!(
        events.iter().any(|(_, topics, data)| {
            topics == (symbol_short!("qupdate"), pid).into_val(&env)
                && i128::try_from_val(&env, &data).ok() == Some(200_i128)
        }),
        "expected 'qupdate' event with new_quorum=200"
    );
}

/// `admin_transferred` — topics: `("admxfer",)`,
/// data: `(old_admin: Address, new_admin: Address)`
#[test]
fn test_event_snapshot_admin_transferred() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let token = snap_token(&env, &admin);
    let c = snap_gov(&env, &admin, &token);
    c.transfer_admin(&admin, &new_admin);

    let events = env.events().all();
    assert!(
        events.iter().any(|(_, topics, data)| {
            topics == (symbol_short!("admxfer"),).into_val(&env)
                && <(Address, Address)>::try_from_val(&env, &data)
                    .ok()
                    .map(|(old, _)| old == admin)
                    .unwrap_or(false)
        }),
        "expected 'admxfer' event with (old_admin, new_admin)"
    );
}

/// `contract_paused` — topics: `("paused",)`, data: `admin: Address`
#[test]
fn test_event_snapshot_contract_paused() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let token = snap_token(&env, &admin);
    let c = snap_gov(&env, &admin, &token);
    c.pause(&admin);

    let events = env.events().all();
    assert!(
        events.iter().any(|(_, topics, data)| {
            topics == (symbol_short!("paused"),).into_val(&env)
                && Address::try_from_val(&env, &data).is_ok()
        }),
        "expected 'paused' event with admin Address"
    );
}

/// `contract_unpaused` — topics: `("unpaused",)`, data: `admin: Address`
#[test]
fn test_event_snapshot_contract_unpaused() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let token = snap_token(&env, &admin);
    let c = snap_gov(&env, &admin, &token);
    c.pause(&admin);
    c.unpause(&admin);

    let events = env.events().all();
    assert!(
        events.iter().any(|(_, topics, data)| {
            topics == (symbol_short!("unpaused"),).into_val(&env)
                && Address::try_from_val(&env, &data).is_ok()
        }),
        "expected 'unpaused' event with admin Address"
    );
}

/// `admin_transfer_proposed` — topics: `("admprop",)`,
/// data: `(current_admin: Address, nominee: Address, expiry: u64)`
#[test]
fn test_event_snapshot_admin_transfer_proposed() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let nominee = Address::generate(&env);
    let token = snap_token(&env, &admin);
    let c = snap_gov(&env, &admin, &token);
    // Use exactly MIN_TRANSFER_WINDOW (300s).
    c.propose_admin_transfer(&admin, &nominee, &300_u64);

    let events = env.events().all();
    assert!(
        events.iter().any(|(_, topics, data)| {
            topics == (symbol_short!("admprop"),).into_val(&env)
                && <(Address, Address, u64)>::try_from_val(&env, &data)
                    .ok()
                    .map(|(a, n, _)| a == admin && n == nominee)
                    .unwrap_or(false)
        }),
        "expected 'admprop' event with (admin, nominee, expiry)"
    );
}
