# Indexer End-to-End Tests

This directory contains the end-to-end test suite for the VoteChain indexer.
These tests spin up a real local Stellar node and PostgreSQL via Docker Compose,
deploy the contracts, emit real contract events, and verify they appear in the
database — the highest-confidence test for the indexer.

## Running Locally

```bash
bash tests/indexer_e2e/e2e_test.sh
```

## CI

These tests run:
- Nightly at 02:00 UTC (scheduled)
- On every push/PR that changes files under `indexer/**`

See `.github/workflows/indexer-e2e.yml`.

## Requirements

- Docker and Docker Compose v2
- Rust toolchain with `wasm32-unknown-unknown` target
- Stellar CLI 22.8.2

## What the Test Does

1. Starts `stellar-node` (local Stellar/Soroban node) and `postgres` via Docker Compose
2. Waits for both services to be healthy
3. Deploys the governance and token contracts to the local node
4. Creates a proposal, casts votes, finalises the proposal
5. Runs the indexer to ingest events
6. Queries PostgreSQL and asserts all expected events are present
7. Tears down all Docker services on exit
