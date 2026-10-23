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

use crate::types::{ConfigKey, ProposalState, Vote};
use soroban_sdk::{symbol_short, Address, Env, String};
/// # Event Schema
///
/// All events are published via `env.events().publish(topics, data)`.
/// Topics are a tuple of `(Symbol, ...)` for efficient off-chain filtering.
///
/// | Function      | Topic 0        | Topic 1       | Data                                          |
/// |---------------|----------------|---------------|-----------------------------------------------|
/// | initialize    | `"init"`       | —             | `admin: Address`                              |
/// | create_proposal | `"created"`  | `id: u64`     | `proposer: Address`                           |
/// | cast_vote     | `"vote"`       | `id: u64`     | `(voter, vote, weight, balance_snapshot)`     |
/// | finalise      | `"final"`      | `id: u64`     | `state: ProposalState`                        |
/// | execute       | `"executed"`   | `id: u64`     | `()`                                          |
/// | cancel        | `"cancelled"`  | `id: u64`     | `()`                                          |
/// | update_quorum | `"qupdate"`    | `id: u64`     | `new_quorum: i128`                            |
/// | transfer_admin | `"admxfer"`   | —             | `(old_admin, new_admin): (Address, Address)` |
/// | extend_voting | `"vextend"`    | `id: u64`     | `(old_end_time, new_end_time): (u64, u64)`   |
/// | cast_vote (comment) | `"vcomment"` | `id: u64` | `(voter, comment_hash): (Address, String)`   |
/// Emits an `init` event when the contract is initialised.
///
/// Topics: `("init",)`  
/// Data: `admin: Address`
pub fn contract_initialized(env: &Env, admin: &Address) {
    env.events()
        .publish((symbol_short!("init"),), admin.clone());
}

/// Emits a `created` event when a new proposal is created.
///
/// Topics: `("created", id)`  
/// Data: `proposer: Address`
pub fn proposal_created(env: &Env, id: u64, proposer: &Address) {
    env.events()
        .publish((symbol_short!("created"), id), proposer.clone());
}

/// Emits a `vote` event when a vote is cast.
///
/// Topics: `("vote", id)`  
/// Data: `(voter: Address, vote: Vote, weight: i128, balance_snapshot: i128)`
pub fn vote_cast(env: &Env, id: u64, voter: &Address, vote: &Vote, weight: i128, balance_snapshot: i128) {
    env.events().publish(
        (symbol_short!("vote"), id),
        (voter.clone(), vote.clone(), weight, balance_snapshot),
    );
}

/// Emits a `final` event when a proposal is finalised (Passed or Rejected).
///
/// Topics: `("final", id)`
/// Data: `(state: ProposalState, execute_after: u64)`
///
/// `execute_after` is the earliest Unix timestamp at which the proposal may be
/// executed (non-zero only when `state == Passed`).  Consumers can use this to
/// schedule an execution call without querying the proposal struct separately.
pub fn proposal_finalised(env: &Env, id: u64, state: &ProposalState, execute_after: u64) {
    env.events()
        .publish((symbol_short!("final"), id), (state.clone(), execute_after));
}

/// Emits an `executed` event when a passed proposal is executed.
///
/// Topics: `("executed", id)`  
/// Data: `()`
pub fn proposal_executed(env: &Env, id: u64) {
    env.events().publish((symbol_short!("executed"), id), ());
}

/// Emits a `cancelled` event when a proposal is cancelled by admin.
///
/// Topics: `("cancelled", id)`  
/// Data: `()`
pub fn proposal_cancelled(env: &Env, id: u64) {
    env.events().publish((symbol_short!("cancelled"), id), ());
}

/// Emits a `qupdate` event when a proposal's quorum is updated.
///
/// Topics: `("qupdate", id)`  
/// Data: `new_quorum: i128`
pub fn quorum_updated(env: &Env, id: u64, new_quorum: i128) {
    env.events()
        .publish((symbol_short!("qupdate"), id), new_quorum);
}

/// Emits an `admxfer` event when admin rights are transferred.
///
/// Topics: `("admxfer",)`
/// Data: `(old_admin: Address, new_admin: Address)`
pub fn admin_transferred(env: &Env, old_admin: &Address, new_admin: &Address) {
    env.events().publish(
        (symbol_short!("admxfer"),),
        (old_admin.clone(), new_admin.clone()),
    );
}

/// Emits an `admpropose` event when a two-step admin rotation is proposed.
///
/// Topics: `("admpropose",)`
/// Data: `(current_admin: Address, nominee: Address, expiry: u64)`
pub fn admin_transfer_proposed(env: &Env, admin: &Address, nominee: &Address, expiry: u64) {
    env.events().publish(
        (symbol_short!("admprop"),),
        (admin.clone(), nominee.clone(), expiry),
    );
}

/// Emits a `paused` event when the contract is paused.
///
/// Topics: `("paused",)`
/// Data: `admin: Address`
pub fn contract_paused(env: &Env, admin: &Address) {
    env.events()
        .publish((symbol_short!("paused"),), admin.clone());
}

/// Emits an `unpaused` event when the contract is unpaused.
///
/// Topics: `("unpaused",)`
/// Data: `admin: Address`
pub fn contract_unpaused(env: &Env, admin: &Address) {
    env.events()
        .publish((symbol_short!("unpaused"),), admin.clone());
}

/// Emits a `delegset` event when a delegator sets a new delegation.
///
/// Topics: `("delegset",)`
/// Data: `(delegator: Address, delegate: Address)`
pub fn delegation_set(env: &Env, delegator: &Address, delegate: &Address) {
    env.events().publish(
        (symbol_short!("delegated"),),
        (delegator.clone(), delegate.clone()),
    );
    env.events().publish(
        (symbol_short!("delegset"),),
        (delegator.clone(), delegate.clone()),
    );
}

/// Emits a `delegrevk` event when a delegator revokes their delegation.
///
/// Topics: `("delegrevk",)`
/// Data: `delegator: Address`
pub fn delegation_revoked(env: &Env, delegator: &Address) {
    env.events()
        .publish((symbol_short!("delegrevk"),), delegator.clone());
}

/// Emits a `paramoveride` event when the admin uses emergency override to change parameters.
///
/// Topics: `("paramoveride",)`
/// Data: `(admin: Address, config_key: ConfigKey, new_value: u64)`
pub fn admin_parameter_override(env: &Env, admin: &Address, config_key: &ConfigKey, new_value: u64) {
    env.events().publish(
        (symbol_short!("paramoveride"),),
        (admin.clone(), config_key.clone(), new_value),
    );
}

/// Emits a `tlupdate` event when the admin updates the timelock duration.
///
/// Topics: `("tlupdate",)`
/// Data: `(admin: Address, old_duration: u64, new_duration: u64)`
///
/// Only applies to proposals created after the update — existing proposals retain the
/// `execute_after` timestamp that was computed when they were finalised.
pub fn timelock_updated(env: &Env, admin: &Address, old_duration: u64, new_duration: u64) {
    env.events().publish(
        (symbol_short!("tlupdate"),),
        (admin.clone(), old_duration, new_duration),
    );
}

/// Emits a `vcomment` event when a voter attaches an off-chain comment hash to their vote.
///
/// Topics: `("vcomment", id)`
/// Data: `(voter: Address, comment_hash: String)`
///
/// Indexers/frontends can subscribe to this topic to resolve and display the
/// linked off-chain comment (e.g. render an IPFS gateway link for the CID).
pub fn vote_comment_attached(env: &Env, id: u64, voter: &Address, comment_hash: &String) {
    env.events().publish(
        (symbol_short!("vcomment"), id),
        (voter.clone(), comment_hash.clone()),
    );
}

/// Emits a `vextend` event when an admin extends a proposal's voting period.
///
/// Topics: `("vextend", id)`
/// Data: `(old_end_time: u64, new_end_time: u64)`
pub fn voting_extended(env: &Env, id: u64, old_end_time: u64, new_end_time: u64) {
    env.events().publish(
        (symbol_short!("vextend"), id),
        (old_end_time, new_end_time),
    );
}

/// Emits a `tmplreg` event when an admin registers a proposal template.
///
/// Topics: `("tmplreg",)`
/// Data: `(admin: Address, template_id: String)`
pub fn template_registered(env: &Env, admin: &Address, template_id: &String) {
    env.events().publish(
        (symbol_short!("tmplreg"),),
        (admin.clone(), template_id.clone()),
    );
}

/// Emits a `tmplrm` event when an admin removes a proposal template.
///
/// Topics: `("tmplrm",)`
/// Data: `(admin: Address, template_id: String)`
pub fn template_removed(env: &Env, admin: &Address, template_id: &String) {
    env.events().publish(
        (symbol_short!("tmplrm"),),
        (admin.clone(), template_id.clone()),
    );
}

/// Emits a `tmclus` event when a proposal is created from a template.
///
/// Topics: `("tmplus", proposal_id)`
/// Data: `(proposer: Address, template_id: String)`
pub fn proposal_from_template(env: &Env, proposal_id: u64, proposer: &Address, template_id: &String) {
    env.events().publish(
        (symbol_short!("tmplus"), proposal_id),
        (proposer.clone(), template_id.clone()),
    );
}
