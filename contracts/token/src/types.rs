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

use soroban_sdk::{contracterror, contracttype, Address};

/// All revert conditions for the token contract.
#[contracterror]
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u32)]
pub enum ContractError {
    /// 1 – Admin address is not set
    AdminNotSet = 1,
    /// 2 – Caller is not the admin
    NotAdmin = 2,
    /// 3 – Transfer/mint/burn amount must be positive
    InvalidAmount = 3,
    /// 4 – Sender has insufficient balance
    InsufficientBalance = 4,
    /// 5 – Spender allowance is insufficient
    AllowanceExceeded = 5,
    /// 6 – New admin address is invalid (zero address)
    InvalidNewAdmin = 6,
    /// 7 – Address parameter is the zero/default address
    InvalidAddress = 7,
    /// 8 – Allowance has expired (current ledger sequence > expiry_ledger)
    AllowanceExpired = 8,
    /// 9 – expiry_ledger must be >= current ledger sequence
    InvalidExpiry = 9,
    /// 10 – SEC-013: transfer/transfer_from amount exceeds the transferable
    /// (unstaked) balance while an active stake lock is in effect
    TokensLocked = 10,
    /// 11 – SEC-013: no active stake position exists for the given address
    StakeNotFound = 11,
    /// 12 – SEC-013: unstake called before `locked_until` has elapsed
    StakeLocked = 12,
    /// 13 – SEC-013: `lock_duration_seconds` exceeds `MAX_LOCK_DAYS` in seconds
    InvalidLockDuration = 13,
    /// 14 – SEC-013: checked arithmetic overflow while computing `locked_until`
    ArithmeticOverflow = 14,
    /// 15 – SEC-013: caller already has an active stake position (v1 supports
    /// only a single position per address)
    AlreadyStaked = 15,
}

/// Per-allowance record storing both the approved amount and the ledger sequence
/// number at which the allowance expires (inclusive).
///
/// Stored in temporary storage keyed by `TokenDataKey::Allowance(owner, spender)`.
/// An allowance is valid while `env.ledger().sequence() <= expiry_ledger`.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Allowance {
    /// Maximum tokens the spender may transfer on behalf of the owner.
    pub amount: i128,
    /// The ledger sequence number at which this allowance expires (inclusive).
    /// After this ledger the allowance is treated as zero by `transfer_from`
    /// and `allowance`.
    pub expiry_ledger: u32,
}

/// Storage key enum for the token contract.
///
/// Every storage entry is keyed by a variant of this enum.  Because Soroban
/// serialises the variant discriminant as part of the XDR key, each variant
/// occupies a completely separate key space — two variants with the same
/// payload can never collide.
///
/// ## Key-space map (SEC-006 collision analysis)
///
/// | Variant                        | Storage tier | Description                              |
/// |-------------------------------|--------------|------------------------------------------|
/// | `Balance(Address)`            | Persistent   | Per-address token balance                |
/// | `Allowance(Address, Address)` | Temporary    | Spender allowance granted by owner       |
/// | `TotalSupply`                 | Instance     | Aggregate token supply                   |
/// | `Admin`                       | Instance     | Contract administrator address           |
/// | `Version`                     | Instance     | Semver tuple `(major, minor, patch)`     |
///
/// ## Collision safety
///
/// Soroban encodes the enum discriminant as the first element of every XDR key.
/// `Balance(Address)` and `Allowance(Address, Address)` both carry `Address`
/// payloads, but their distinct discriminants ensure they can never alias.
/// Singleton variants (`Admin`, `TotalSupply`, `Version`) have no payload and
/// are unconditionally unique within this contract.
#[contracttype]
pub enum TokenDataKey {
    /// Per-address token balance (persistent storage).
    /// Key space: one entry per unique holder address.
    Balance(Address),

    /// Spender allowance granted by `owner` to `spender` (temporary storage).
    /// Key space: one entry per `(owner, spender)` pair; expires with the ledger.
    Allowance(Address, Address),

    /// Aggregate token supply across all holders (instance storage).
    /// Key space: singleton — only one `TotalSupply` entry exists.
    TotalSupply,

    /// Contract administrator address (instance storage).
    /// Key space: singleton — only one `Admin` entry exists.
    Admin,

    /// Contract version stored as a `(major, minor, patch)` semver tuple (instance storage).
    /// Key space: singleton — only one `Version` entry exists.
    Version,

    /// SEC-013: Active ve-token stake position for `Address` (persistent storage).
    /// Key space: one entry per staker; removed on `unstake`.
    StakeRecord(Address),
}
