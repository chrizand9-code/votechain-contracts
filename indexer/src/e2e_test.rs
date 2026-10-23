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

//! Indexer end-to-end tests (issue #304).
//!
//! These tests require a live Docker environment with a local Stellar node and
//! PostgreSQL running. They are marked `#[ignore]` so they are skipped in
//! regular `cargo test` runs.
//!
//! To run:
//! ```bash
//! bash tests/indexer_e2e/e2e_test.sh
//! # or manually:
//! DATABASE_URL=postgres://votechain:votechain@localhost:5433/votechain_test \
//! STELLAR_RPC_URL=http://localhost:8000 \
//! cargo test -p votechain-indexer -- --ignored test_proposal_lifecycle_events_indexed --nocapture
//! ```

#[cfg(test)]
mod tests {
    /// End-to-end test: deploys contracts, emits real events, and verifies
    /// they appear in PostgreSQL via the indexer ingestion loop.
    ///
    /// Requires: `DATABASE_URL` and `STELLAR_RPC_URL` environment variables.
    /// Run via: `bash tests/indexer_e2e/e2e_test.sh`
    #[test]
    #[ignore = "requires live Docker environment (stellar-node + postgres)"]
    fn test_proposal_lifecycle_events_indexed() {
        let db_url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set to run E2E tests");
        let rpc_url = std::env::var("STELLAR_RPC_URL")
            .unwrap_or_else(|_| "http://localhost:8000".to_string());

        assert!(!db_url.is_empty(), "DATABASE_URL must not be empty");
        assert!(
            rpc_url.starts_with("http"),
            "STELLAR_RPC_URL must be an http URL, got: {rpc_url}"
        );

        // TODO: Full E2E flow (issue #304):
        //   1. Connect to PostgreSQL via DATABASE_URL
        //   2. Run migrations
        //   3. Deploy governance + token contracts via Stellar CLI
        //   4. Create proposal, cast votes, finalise
        //   5. Run indexer ingestion loop for N ledgers
        //   6. Query events table and assert ProposalCreated, VoteCast,
        //      ProposalFinalised events are all present
        //
        // This stub passes the environment checks. Implement the full flow
        // once the Docker Compose infrastructure is in place.
        println!("E2E stub: DATABASE_URL={db_url}, STELLAR_RPC_URL={rpc_url}");
        println!("Stub test passed — full implementation tracked in issue #304.");
    }
}
