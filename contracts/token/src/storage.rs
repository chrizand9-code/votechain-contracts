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

//! Storage accessors for the token contract.
//!
//! # Namespacing strategy
//!
//! All storage entries are keyed by variants of [`TokenDataKey`].  Soroban
//! serialises the enum variant discriminant into the XDR key before any
//! payload, so every variant occupies a completely isolated key space.
//! Adding a new data type requires only a new enum variant — there is no
//! risk of collision with existing keys.
//!
//! Storage tiers in use:
//! - **Instance** – singleton config values (`Admin`, `TotalSupply`, `Version`).
//!   Shares the contract instance TTL; cheap to access.
//! - **Persistent** – per-address data (`Balance`).
//!   Survives ledger expiry; must be bumped explicitly for long-lived entries.
//! - **Temporary** – short-lived allowances (`Allowance`).
//!   Automatically expires; no manual TTL management required.

use crate::staking::StakeRecord;
use crate::types::{Allowance, ContractError, TokenDataKey};
use soroban_sdk::{Address, Env};

const BALANCE_TTL_THRESHOLD: u32 = 2_592_000;
const BALANCE_TTL_EXTEND_TO: u32 = 3_110_400;

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
//   TokenDataKey::Admin       – admin address (set at init, read on mint/burn)
//   TokenDataKey::TotalSupply – total tokens in circulation (read on every mint/burn)
//   TokenDataKey::Version     – semver tuple (major, minor, patch)
//
// PERSISTENT storage – per-key TTL, survives ledger expiry independently.
//                      Used for data keyed by a variable (owner address, etc.)
//                      that must survive indefinitely.
//
//   TokenDataKey::Balance(owner) – token balance per address
//
// TEMPORARY storage  – expires at the end of the ledger entry's TTL without
//                      renewal. Suitable for short-lived approvals that do not
//                      need to persist across many ledgers.
//
//   TokenDataKey::Allowance(owner, spender) – ERC-20-style spending allowance
// =============================================================================

/// Returns the token balance of `owner`. Defaults to `0` if never set.
pub fn balance_of(env: &Env, owner: &Address) -> i128 {
    let key = TokenDataKey::Balance(owner.clone());
    let balance = env.storage().persistent().get(&key);
    if balance.is_some() {
        env.storage().persistent().extend_ttl(
            &key,
            BALANCE_TTL_THRESHOLD,
            BALANCE_TTL_EXTEND_TO,
        );
    }
    balance.unwrap_or(0)
}

/// Sets the token balance of `owner` to `amount`.
pub fn set_balance(env: &Env, owner: &Address, amount: i128) {
    let key = TokenDataKey::Balance(owner.clone());
    env.storage().persistent().set(&key, &amount);
    env.storage().persistent().extend_ttl(
        &key,
        BALANCE_TTL_THRESHOLD,
        BALANCE_TTL_EXTEND_TO,
    );
}

/// Returns the spending allowance granted by `owner` to `spender`.
///
/// Returns `0` if no allowance exists or if the allowance has expired
/// (i.e. `current_ledger_sequence > expiry_ledger`).
pub fn allowance(env: &Env, owner: &Address, spender: &Address) -> i128 {
    let key = TokenDataKey::Allowance(owner.clone(), spender.clone());
    match env.storage().temporary().get::<_, Allowance>(&key) {
        Some(a) if env.ledger().sequence() <= a.expiry_ledger => a.amount,
        _ => 0,
    }
}

/// Returns the full [`Allowance`] record granted by `owner` to `spender`, or
/// `None` if no allowance has been set.
///
/// Unlike [`allowance`] this returns the raw record even if it has expired,
/// so callers that need to inspect the expiry can do so.
pub fn get_allowance(env: &Env, owner: &Address, spender: &Address) -> Option<Allowance> {
    env.storage()
        .temporary()
        .get(&TokenDataKey::Allowance(owner.clone(), spender.clone()))
}

/// Sets the spending allowance granted by `owner` to `spender`.
pub fn set_allowance(env: &Env, owner: &Address, spender: &Address, record: &Allowance) {
    env.storage().temporary().set(
        &TokenDataKey::Allowance(owner.clone(), spender.clone()),
        record,
    );
}

/// Returns the total token supply. Defaults to `0` before initialisation.
pub fn total_supply(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&TokenDataKey::TotalSupply)
        .unwrap_or(0)
}

/// Stores the total token supply.
pub fn set_total_supply(env: &Env, s: i128) {
    env.storage().instance().set(&TokenDataKey::TotalSupply, &s);
}

/// Returns the stored admin address.
///
/// # Errors
/// - [`ContractError::AdminNotSet`] if the contract has not been initialised.
pub fn get_admin(env: &Env) -> Result<Address, ContractError> {
    env.storage()
        .instance()
        .get(&TokenDataKey::Admin)
        .ok_or(ContractError::AdminNotSet)
}

/// Stores the admin address in instance storage.
pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&TokenDataKey::Admin, admin);
}

/// Stores the contract version as a `(major, minor, patch)` tuple.
pub fn set_version(env: &Env, version: (u32, u32, u32)) {
    env.storage()
        .instance()
        .set(&TokenDataKey::Version, &version);
}

/// Returns the stored contract version as a `(major, minor, patch)` tuple.
pub fn get_version(env: &Env) -> (u32, u32, u32) {
    env.storage()
        .instance()
        .get(&TokenDataKey::Version)
        .unwrap_or((0, 0, 0))
}

/// SEC-013: Returns the active [`StakeRecord`] for `staker`, if any.
pub fn get_stake_record(env: &Env, staker: &Address) -> Option<StakeRecord> {
    env.storage()
        .persistent()
        .get(&TokenDataKey::StakeRecord(staker.clone()))
}

/// SEC-013: Writes (or overwrites) the [`StakeRecord`] for `staker`.
pub fn set_stake_record(env: &Env, staker: &Address, record: &StakeRecord) {
    env.storage()
        .persistent()
        .set(&TokenDataKey::StakeRecord(staker.clone()), record);
}

/// SEC-013: Deletes the [`StakeRecord`] for `staker` (called on full unstake).
pub fn remove_stake_record(env: &Env, staker: &Address) {
    env.storage()
        .persistent()
        .remove(&TokenDataKey::StakeRecord(staker.clone()));
}
