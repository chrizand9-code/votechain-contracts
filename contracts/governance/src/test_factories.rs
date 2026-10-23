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

//! Test data factories for governance test scenarios (issue #306).
//!
//! These factories reduce boilerplate in test files by providing
//! composable, well-named scenario builders. They are more comprehensive
//! than the lower-level helpers in `test_helpers.rs`.
//!
//! # Usage
//!
//! ```rust
//! use crate::test_factories::*;
//!
//! #[test]
//! fn my_test() {
//!     let (t, pid) = default_proposal();
//!     // t is a fully initialised TestEnv, pid is the proposal id
//! }
//! ```

#![cfg(test)]

use crate::test_helpers::{create_test_proposal, mint_and_vote, setup_env, TestEnv};
use crate::types::Vote;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Address;

// ── Basic environment factory ────────────────────────────────────────────────

/// Returns a [`TestEnv`] ready for testing — governance + token deployed,
/// admin has 10_000_000 tokens. This is an alias for [`setup_env`] and is
/// the starting point for all other factories.
///
/// # Example
/// ```rust
/// let t = default_env();
/// assert_eq!(t.client.proposal_count(), 0);
/// ```
pub fn default_env() -> TestEnv {
    setup_env()
}

// ── Proposal factories ───────────────────────────────────────────────────────

/// Creates a [`TestEnv`] with one proposal in `Active` state.
///
/// The proposal uses sensible defaults:
/// - title: "Test proposal"
/// - description: "Test description"
/// - quorum: 100 tokens
/// - duration: 3600 seconds (1 hour)
///
/// Returns `(TestEnv, proposal_id)`.
pub fn default_proposal() -> (TestEnv, u64) {
    let t = setup_env();
    let pid = create_test_proposal(&t, &t.admin.clone());
    (t, pid)
}

// ── Vote scenario factories ──────────────────────────────────────────────────

/// Scenario: one voter mints 1_000 tokens and casts a **Yes** vote.
///
/// With quorum = 100 and votes_yes = 1_000 > votes_no = 0, this proposal
/// will **pass** when finalised after the voting period ends.
///
/// Returns `(TestEnv, proposal_id, voter_address)`.
pub fn passing_vote_scenario() -> (TestEnv, u64, Address) {
    let t = setup_env();
    let pid = create_test_proposal(&t, &t.admin.clone());
    let voter = Address::generate(&t.env);
    mint_and_vote(&t, &voter, pid, Vote::Yes, 1_000);
    (t, pid, voter)
}

/// Scenario: same as [`passing_vote_scenario`] but the voter casts **Abstain**.
///
/// Abstain votes count toward quorum (total_votes = 1_000 >= quorum = 100)
/// but do **not** count toward the yes/no majority. Since votes_yes = 0 is
/// not greater than votes_no = 0, the proposal is **rejected** even though
/// quorum is met.
///
/// Returns `(TestEnv, proposal_id, voter_address)`.
pub fn passing_vote_scenario_with_abstain() -> (TestEnv, u64, Address) {
    let t = setup_env();
    let pid = create_test_proposal(&t, &t.admin.clone());
    let voter = Address::generate(&t.env);
    mint_and_vote(&t, &voter, pid, Vote::Abstain, 1_000);
    (t, pid, voter)
}

/// Scenario: no votes are cast, so quorum is **not met**.
///
/// When finalised the proposal will be **rejected** (total_votes = 0 < quorum = 100).
///
/// Returns `(TestEnv, proposal_id)`.
pub fn quorum_not_met_scenario() -> (TestEnv, u64) {
    let t = setup_env();
    let pid = create_test_proposal(&t, &t.admin.clone());
    (t, pid)
}

/// Scenario: a proposal is **cancelled** by the admin while still Active.
///
/// The returned proposal is in `Cancelled` state.
///
/// Returns `(TestEnv, proposal_id)`.
pub fn cancelled_proposal() -> (TestEnv, u64) {
    let t = setup_env();
    let pid = create_test_proposal(&t, &t.admin.clone());
    t.client.cancel(&t.admin, &pid);
    (t, pid)
}
