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

/**
 * Consumer-driven contract test (issue #308) between the SDK (consumer) and
 * the backend (provider), using Pact.
 *
 * Scope note: as of this writing, `backend/src/routes/proposals.ts` exposes
 * `GET /proposals` and `GET /proposals/:id` only. There is no vote-related
 * HTTP endpoint in the backend — on-chain voting is performed directly
 * against the Soroban contract via `GovernanceClient` (see `../client.ts`),
 * not through a backend HTTP route. This contract therefore covers the
 * proposal-read endpoints that genuinely exist today; see
 * `docs/testing/pact-contract-testing.md` for the plan to extend coverage
 * once (if) a vote-related backend endpoint is added.
 *
 * This file is intentionally excluded from the default `npm test` run
 * (`vitest.config.ts` only includes `src/**\/*.test.ts`) because it depends
 * on `@pact-foundation/pact`, which is declared in `package.json`
 * `devDependencies` but not yet installed/locked in this change (see the doc
 * above for why). Run it explicitly via `npm run test:contract` once that
 * dependency has been installed.
 */

import { describe, it } from "vitest";
// NOTE: requires `@pact-foundation/pact` to be installed (see package.json).
// eslint-disable-next-line import/no-unresolved
import { PactV3, MatchersV3 } from "@pact-foundation/pact";
import path from "node:path";

const { like, integer, eachLike } = MatchersV3;

const provider = new PactV3({
  consumer: "votechain-sdk",
  provider: "votechain-backend",
  dir: path.resolve(__dirname, "../../pacts"),
  logLevel: "warn",
});

describe("Pact: SDK <-> backend /proposals contract", () => {
  it("GET /proposals returns a paginated proposal list matching the SDK's expected shape", async () => {
    provider
      .given("at least one proposal exists")
      .uponReceiving("a request for the proposal list")
      .withRequest({
        method: "GET",
        path: "/proposals",
      })
      .willRespondWith({
        status: 200,
        headers: { "Content-Type": "application/json; charset=utf-8" },
        body: {
          data: eachLike({
            id: integer(1),
            title: like("Example proposal"),
            description: like("Example description"),
            state: like("active"),
            proposal_tags: eachLike(like("treasury")),
          }),
          pagination: {
            limit: integer(20),
            has_more: like(false),
          },
        },
      });

    await provider.executeTest(async (mockServer) => {
      const res = await fetch(`${mockServer.url}/proposals`);
      const json = (await res.json()) as {
        data: Array<{ id: number; title?: string; state?: string }>;
        pagination: { limit: number; has_more: boolean };
      };

      if (res.status !== 200) {
        throw new Error(`expected 200, got ${res.status}`);
      }
      if (!Array.isArray(json.data)) {
        throw new Error("expected `data` to be an array");
      }
      if (typeof json.pagination.limit !== "number") {
        throw new Error("expected `pagination.limit` to be a number");
      }
    });
  });

  it("GET /proposals/:id returns an object containing the requested id", async () => {
    provider
      .given("proposal 42 exists")
      .uponReceiving("a request for a single proposal by id")
      .withRequest({
        method: "GET",
        path: "/proposals/42",
      })
      .willRespondWith({
        status: 200,
        headers: { "Content-Type": "application/json; charset=utf-8" },
        body: {
          id: like("42"),
        },
      });

    await provider.executeTest(async (mockServer) => {
      const res = await fetch(`${mockServer.url}/proposals/42`);
      const json = (await res.json()) as { id: string };

      if (res.status !== 200) {
        throw new Error(`expected 200, got ${res.status}`);
      }
      if (json.id !== "42") {
        throw new Error(`expected id "42", got ${JSON.stringify(json.id)}`);
      }
    });
  });
});
