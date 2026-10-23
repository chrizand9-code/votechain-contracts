#!/usr/bin/env bash
# Copyright 2024 VoteChain Contributors
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

# Indexer end-to-end test runner.
# Starts Docker Compose services, waits for health, runs the E2E test,
# then tears down on exit.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

DC="docker compose -f docker-compose.yml -f tests/indexer_e2e/docker-compose.test.yml"

cleanup() {
  echo "==> Tearing down Docker Compose services..."
  $DC down --volumes --remove-orphans || true
}
trap cleanup EXIT

echo "==> Starting Docker Compose services for indexer E2E test..."
$DC up -d stellar-node postgres

echo "==> Waiting for services to be healthy..."
$DC up --wait stellar-node postgres

echo "==> Running indexer E2E tests (cargo test --ignored)..."
DATABASE_URL="postgres://votechain:votechain@localhost:5433/votechain_test" \
STELLAR_RPC_URL="http://localhost:8000" \
cargo test -p votechain-indexer -- --ignored test_proposal_lifecycle_events_indexed --nocapture

echo "==> Indexer E2E tests passed."
