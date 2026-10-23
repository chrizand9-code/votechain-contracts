# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- **SEC-013** (#316): Implemented the ve-token staking lock (`stake`,
  `unstake`, `staked_voting_power` in `contracts/token/src/staking.rs`) and
  wired lock enforcement into `TokenContract::transfer` and
  `TokenContract::transfer_from`, closing the allowance-trick lock-bypass
  attack described in `docs/security/SEC-013-staking-lock-bypass.md`.

## [0.1.0] - 2026-04-27

### Added

- Governance contract with `initialize`, `create_proposal`, `cast_vote`, `finalise`,
  `execute`, `cancel`, `get_proposal`, `has_voted`, and `proposal_count`
- `update_quorum` — admin function to adjust the quorum threshold on an active proposal
- `get_version` on both contracts returning a `(major, minor, patch)` semver tuple
- Token contract with `initialize`, `transfer`, `approve`, `transfer_from`, `mint`,
  `burn`, `total_supply`, and `balance`
- Three-way voting model: Yes / No / Abstain
- Token-weighted voting using the voter's live on-chain balance at vote time (no snapshots)
- Double-vote prevention via persistent `HasVoted(proposal_id, voter)` storage key
- Proposal lifecycle: Active → Passed / Rejected → Executed, or Active → Cancelled
- On-chain events for all state transitions: `created`, `vote`, `final`, `qupdate`
- `ContractError` enum with 14 typed revert conditions and `#[contracterror]`
- `VoteTallyOverflow` protection using `checked_add` on all vote accumulators
- Spam prevention: configurable minimum proposer token balance and per-address
  proposal cooldown at initialisation time
- Re-initialisation guard — `AlreadyInitialized` error on any duplicate `initialize` call
- Makefile targets: `build`, `test`, `fmt`, `fmt-check`, `lint`, `clean`, `deploy-testnet`
- Docker Compose development environment with a local Stellar node (Soroban RPC)
- GitHub Actions CI: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`,
  `cargo audit`, WASM build, and binary-size enforcement (governance ≤ 100 KB, token ≤ 50 KB)
- WASM validity smoke-test workflow (`scripts/test_wasm.sh`)
- Automated semver patch-bump tagging on every push to `main`
- Release workflow publishing WASM artifacts to GitHub Releases
- Environment config files (`config/local.toml`, `config/testnet.toml`, `config/mainnet.toml`)
- Deploy scripts (`scripts/deploy.sh`, `scripts/deploy_mainnet.sh`)
- Property-based tests (`prop_tests.rs`) and deterministic test snapshots
- Architecture Decision Records: ADR-001 through ADR-005
- Documentation: lifecycle state diagram, FAQ, upgrading guide, error reference,
  getting-started guide, roadmap
- Security documentation: threat model, audit scope, known-issues register,
  `SECURITY.md` responsible-disclosure policy, and `AUDIT.md` checklist
- `cargo doc` API documentation for every public function in both contracts
- `CONTRIBUTING.md` development guide
- `CHANGELOG.md` following Keep a Changelog format with automated release updates

[Unreleased]: https://github.com/veracindarella/votechain-contracts/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/veracindarella/votechain-contracts/releases/tag/v0.1.0
