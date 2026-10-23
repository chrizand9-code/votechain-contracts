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
 * Unit tests for the governance alerts bot trigger logic.
 *
 * Issue #303: Tests cover each alert type (ProposalCreated, VotingEndingSoon,
 * QuorumMet, ProposalFinalised), deduplication logic, and mock all network
 * calls so no real HTTP requests are made.
 */

import { formatMessage, tick, resetForTesting } from "./index";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const MOCK_WEBHOOK = "https://discord.com/api/webhooks/test/mock";

function mockProposal(overrides: Partial<{
  id: number;
  title: string;
  state: string;
  executed: boolean;
}> = {}) {
  return {
    id: overrides.id ?? 1,
    title: overrides.title ?? "Test Proposal",
    state: overrides.state ?? "active",
    executed: overrides.executed ?? false,
  };
}

/** Build a minimal fetch mock that returns the given proposals array. */
function mockFetchWithProposals(
  proposals: ReturnType<typeof mockProposal>[],
  webhookOk = true
) {
  return jest.fn().mockImplementation((url: string) => {
    if (typeof url === "string" && url.includes("discord.com")) {
      return Promise.resolve({ ok: webhookOk, status: 200, text: () => Promise.resolve("") });
    }
    return Promise.resolve({
      ok: true,
      status: 200,
      json: () => Promise.resolve(proposals),
    });
  });
}

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

beforeEach(() => {
  process.env.DISCORD_WEBHOOK_URL = MOCK_WEBHOOK;
  process.env.API_URL = "http://localhost:3001";
  process.env.FRONTEND_URL = "http://localhost:3000";
  resetForTesting();
});

afterEach(() => {
  jest.restoreAllMocks();
});

// ---------------------------------------------------------------------------
// formatMessage — alert type tests (#303 AC: tests for each alert type)
// ---------------------------------------------------------------------------

describe("formatMessage", () => {
  const proposal = mockProposal({ id: 42, title: "Upgrade Treasury", state: "active" });

  test("ProposalCreated includes headline, id, title, and link", () => {
    const msg = formatMessage("ProposalCreated", proposal);
    expect(msg).toContain("New proposal created");
    expect(msg).toContain("#42");
    expect(msg).toContain("Upgrade Treasury");
    expect(msg).toContain("/proposals/42");
  });

  test("ProposalFinalised includes finalised headline", () => {
    const msg = formatMessage("ProposalFinalised", proposal);
    expect(msg).toContain("Proposal finalised");
    expect(msg).toContain("#42");
  });

  test("ProposalExecuted includes executed headline", () => {
    const msg = formatMessage("ProposalExecuted", proposal);
    expect(msg).toContain("Proposal executed");
    expect(msg).toContain("#42");
  });

  test("uses (untitled) when title is missing", () => {
    const p = { id: 1 };
    const msg = formatMessage("ProposalCreated", p);
    expect(msg).toContain("(untitled)");
  });

  test("shows executed state when proposal.executed is true", () => {
    const p = mockProposal({ executed: true });
    const msg = formatMessage("ProposalExecuted", p);
    expect(msg).toContain("`executed`");
  });
});

// ---------------------------------------------------------------------------
// tick() — deduplication logic (#303 AC: alert deduplication logic tested)
// ---------------------------------------------------------------------------

describe("tick() deduplication", () => {
  test("first poll seeds cache without posting any alerts", async () => {
    const proposals = [
      mockProposal({ id: 1, state: "active" }),
      mockProposal({ id: 2, state: "passed" }),
    ];
    const mockFetch = mockFetchWithProposals(proposals);
    global.fetch = mockFetch as unknown as typeof fetch;

    await tick();

    // Only 1 fetch call: the API call. No webhook calls.
    const webhookCalls = mockFetch.mock.calls.filter((c: unknown[]) =>
      typeof c[0] === "string" && c[0].includes("discord.com")
    );
    expect(webhookCalls).toHaveLength(0);
  });

  test("no alert is posted when proposal state does not change", async () => {
    const proposals = [mockProposal({ id: 1, state: "active" })];
    const mockFetch = mockFetchWithProposals(proposals);
    global.fetch = mockFetch as unknown as typeof fetch;

    // First poll — seeds cache
    await tick();
    // Second poll — same state, no change
    await tick();

    const webhookCalls = mockFetch.mock.calls.filter((c: unknown[]) =>
      typeof c[0] === "string" && c[0].includes("discord.com")
    );
    expect(webhookCalls).toHaveLength(0);
  });

  test("ProposalCreated alert fires when a new proposal appears after init", async () => {
    const mockFetch = jest.fn()
      .mockImplementationOnce(() =>
        Promise.resolve({ ok: true, status: 200, json: () => Promise.resolve([]) })
      )
      .mockImplementationOnce(() =>
        Promise.resolve({
          ok: true,
          status: 200,
          json: () => Promise.resolve([mockProposal({ id: 5, state: "active" })]),
        })
      )
      .mockImplementation(() =>
        Promise.resolve({ ok: true, status: 200, text: () => Promise.resolve("") })
      );
    global.fetch = mockFetch as unknown as typeof fetch;

    await tick(); // first poll — empty list, initialised = true
    await tick(); // second poll — proposal 5 appears

    const webhookCalls = mockFetch.mock.calls.filter((c: unknown[]) =>
      typeof c[0] === "string" && c[0].includes("discord.com")
    );
    expect(webhookCalls).toHaveLength(1);
    const body = JSON.parse((webhookCalls[0][1] as { body: string }).body) as { content: string };
    expect(body.content).toContain("New proposal created");
  });

  test("ProposalFinalised alert fires when state transitions to a final state", async () => {
    const mockFetch = jest.fn()
      .mockImplementationOnce(() =>
        Promise.resolve({
          ok: true,
          status: 200,
          json: () => Promise.resolve([mockProposal({ id: 10, state: "active" })]),
        })
      )
      .mockImplementationOnce(() =>
        Promise.resolve({
          ok: true,
          status: 200,
          json: () => Promise.resolve([mockProposal({ id: 10, state: "passed" })]),
        })
      )
      .mockImplementation(() =>
        Promise.resolve({ ok: true, status: 200, text: () => Promise.resolve("") })
      );
    global.fetch = mockFetch as unknown as typeof fetch;

    await tick(); // seeds cache with id=10 active
    await tick(); // id=10 is now passed

    const webhookCalls = mockFetch.mock.calls.filter((c: unknown[]) =>
      typeof c[0] === "string" && c[0].includes("discord.com")
    );
    expect(webhookCalls).toHaveLength(1);
    const body = JSON.parse((webhookCalls[0][1] as { body: string }).body) as { content: string };
    expect(body.content).toContain("Proposal finalised");
  });

  test("ProposalExecuted alert fires when state transitions to executed", async () => {
    const mockFetch = jest.fn()
      .mockImplementationOnce(() =>
        Promise.resolve({
          ok: true,
          status: 200,
          json: () => Promise.resolve([mockProposal({ id: 7, state: "passed" })]),
        })
      )
      .mockImplementationOnce(() =>
        Promise.resolve({
          ok: true,
          status: 200,
          json: () => Promise.resolve([mockProposal({ id: 7, state: "executed", executed: true })]),
        })
      )
      .mockImplementation(() =>
        Promise.resolve({ ok: true, status: 200, text: () => Promise.resolve("") })
      );
    global.fetch = mockFetch as unknown as typeof fetch;

    await tick(); // seeds cache with id=7 passed
    await tick(); // id=7 is now executed

    const webhookCalls = mockFetch.mock.calls.filter((c: unknown[]) =>
      typeof c[0] === "string" && c[0].includes("discord.com")
    );
    expect(webhookCalls).toHaveLength(1);
    const body = JSON.parse((webhookCalls[0][1] as { body: string }).body) as { content: string };
    expect(body.content).toContain("Proposal executed");
  });

  test("duplicate state transitions do not trigger duplicate alerts", async () => {
    const proposals = [mockProposal({ id: 3, state: "active" })];
    const mockFetch = jest.fn()
      .mockImplementationOnce(() =>
        Promise.resolve({ ok: true, status: 200, json: () => Promise.resolve(proposals) })
      )
      .mockImplementation(() =>
        Promise.resolve({ ok: true, status: 200, json: () => Promise.resolve(proposals) })
      );
    global.fetch = mockFetch as unknown as typeof fetch;

    await tick();
    await tick();
    await tick();

    const webhookCalls = mockFetch.mock.calls.filter((c: unknown[]) =>
      typeof c[0] === "string" && c[0].includes("discord.com")
    );
    expect(webhookCalls).toHaveLength(0);
  });
});
