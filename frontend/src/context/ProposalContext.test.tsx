/**
 * Unit tests for ProposalContext state transitions — Issue #302
 *
 * Covers:
 *  1. Initial load — exposes proposals after fetch
 *  2. Loading state — starts true, becomes false after fetch
 *  3. Error state — sets error when data access throws
 *  4. refresh() — re-fetches proposals on demand
 *
 * Strategy:
 *  - `vi.hoisted` creates shared mutable state that the hoisted `vi.mock` factory
 *    can close over (vi.mock factories are hoisted above imports).
 *  - The mock makes `sampleProposals` a getter so individual tests can flip
 *    `shouldThrow.value` to exercise the error branch.
 *  - `vi.useFakeTimers()` / `vi.useRealTimers()` control the 400 ms setTimeout
 *    inside `fetchProposals` without making tests slow.
 */

import { renderHook, act, waitFor } from '@testing-library/react';
import React from 'react';
import { ProposalProvider, useProposals } from './ProposalContext';
import type { Proposal } from '../types';

// ---------------------------------------------------------------------------
// Shared hoisted state — must be created with vi.hoisted so it is available
// inside the vi.mock factory which runs before any import statements.
// ---------------------------------------------------------------------------
const { shouldThrow, mockProposals } = vi.hoisted(() => {
  const mockProposals: Proposal[] = [
    {
      id: 'T-1',
      title: 'Alpha Proposal',
      description: 'First test proposal',
      proposer: 'GABC1',
      state: 'Active',
      createdAt: '2026-01-01',
      endAt: '2026-02-01',
      quorum: 100,
      votesCount: 5,
      totalWeight: 500,
      votes: [],
    },
    {
      id: 'T-2',
      title: 'Beta Proposal',
      description: 'Second test proposal',
      proposer: 'GABC2',
      state: 'Passed',
      createdAt: '2026-01-02',
      endAt: '2026-02-02',
      quorum: 200,
      votesCount: 10,
      totalWeight: 1000,
      votes: [],
    },
  ];
  return { shouldThrow: { value: false }, mockProposals };
});

// ---------------------------------------------------------------------------
// Mock the data module.  Using a getter for `sampleProposals` means the value
// is re-evaluated on every access — this is required so the error test can
// change `shouldThrow.value` after module evaluation.
// ---------------------------------------------------------------------------
vi.mock('../data', () => ({
  get sampleProposals() {
    if (shouldThrow.value) {
      throw new Error('Network error');
    }
    return mockProposals;
  },
}));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Wrapper that provides the ProposalProvider context to hooks under test. */
function wrapper({ children }: { children: React.ReactNode }) {
  return <ProposalProvider>{children}</ProposalProvider>;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

beforeEach(() => {
  // Reset throw flag so each test starts in a clean state.
  shouldThrow.value = false;
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

// --- Test 1 ----------------------------------------------------------------
test('initial load — exposes proposals after fetch', async () => {
  const { result } = renderHook(() => useProposals(), { wrapper });

  // Advance fake timers past the 400 ms setTimeout in fetchProposals.
  await act(async () => {
    await vi.advanceTimersByTimeAsync(500);
  });

  await waitFor(() => {
    expect(result.current.loading).toBe(false);
  });

  expect(result.current.proposals.length).toBeGreaterThan(0);
  expect(result.current.proposals).toHaveLength(mockProposals.length);
  expect(result.current.error).toBeNull();
});

// --- Test 2 ----------------------------------------------------------------
test('loading state — starts true, becomes false after load', async () => {
  const { result } = renderHook(() => useProposals(), { wrapper });

  // Before timers fire the hook should still be loading.
  expect(result.current.loading).toBe(true);

  await act(async () => {
    await vi.advanceTimersByTimeAsync(500);
  });

  await waitFor(() => {
    expect(result.current.loading).toBe(false);
  });
});

// --- Test 3 ----------------------------------------------------------------
test('error state — sets error when data throws', async () => {
  // Activate the throw path inside the sampleProposals getter.
  shouldThrow.value = true;

  const { result } = renderHook(() => useProposals(), { wrapper });

  await act(async () => {
    await vi.advanceTimersByTimeAsync(500);
  });

  await waitFor(() => {
    expect(result.current.loading).toBe(false);
  });

  expect(result.current.error).not.toBeNull();
  expect(result.current.error).toContain('Network error');
  expect(result.current.proposals).toHaveLength(0);
});

// --- Test 4 ----------------------------------------------------------------
test('refresh — re-fetches proposals on demand', async () => {
  const { result } = renderHook(() => useProposals(), { wrapper });

  // Wait for initial load to complete.
  await act(async () => {
    await vi.advanceTimersByTimeAsync(500);
  });
  await waitFor(() => expect(result.current.loading).toBe(false));

  const firstLoad = result.current.proposals;

  // Call refresh() and advance timers again.
  await act(async () => {
    result.current.refresh();
    await vi.advanceTimersByTimeAsync(500);
  });

  await waitFor(() => expect(result.current.loading).toBe(false));

  // After a successful refresh the proposals should still be present.
  expect(result.current.proposals).toHaveLength(firstLoad.length);
  expect(result.current.error).toBeNull();
});
