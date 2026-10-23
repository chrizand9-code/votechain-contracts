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
 * ProposalContext — global proposal data store (issue #10).
 *
 * Provides: proposals, loading, error, refresh()
 * Fetches proposals once and makes them available application-wide
 * via <ProposalProvider> in main.tsx.
 */
import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
} from 'react';
import type { Proposal } from '../types';
import { sampleProposals } from '../data';

// ── Types ────────────────────────────────────────────────────

export interface ProposalContextValue {
  /** All loaded proposals */
  proposals: Proposal[];
  /** True while the initial or refresh fetch is in flight */
  loading: boolean;
  /** Fetch error message, or null */
  error: string | null;
  /** Manually trigger a re-fetch */
  refresh: () => Promise<void>;
}

// ── Context ──────────────────────────────────────────────────

const ProposalContext = createContext<ProposalContextValue | null>(null);

// ── Fetcher (replace with real API call) ─────────────────────

async function fetchProposals(): Promise<Proposal[]> {
  // TODO: replace with `await fetch('/api/proposals').then(r => r.json())`
  // Using sample data until the backend endpoint is wired up.
  return new Promise((resolve) => setTimeout(() => resolve(sampleProposals), 400));
}

// ── Provider ─────────────────────────────────────────────────

interface ProposalProviderProps {
  children: React.ReactNode;
  /**
   * Optional fetcher override — injected by unit tests to control the data
   * source without MSW or network calls (issue #302).
   * Defaults to the internal `fetchProposals` function.
   */
  fetcher?: () => Promise<Proposal[]>;
}

export function ProposalProvider({ children, fetcher }: ProposalProviderProps) {
  const [proposals, setProposals] = useState<Proposal[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = fetcher ?? fetchProposals;

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await load();
      setProposals(data);
    } catch (e: unknown) {
      setError((e as { message?: string })?.message ?? 'Failed to fetch proposals.');
    } finally {
      setLoading(false);
    }
  }, [load]);

  // Fetch on mount
  useEffect(() => {
    refresh();
  }, [refresh]);

  const value: ProposalContextValue = { proposals, loading, error, refresh };

  return (
    <ProposalContext.Provider value={value}>{children}</ProposalContext.Provider>
  );
}

// ── Hook ─────────────────────────────────────────────────────

/**
 * useProposals — consume ProposalContext.
 * Must be used inside <ProposalProvider>.
 */
export function useProposals(): ProposalContextValue {
  const ctx = useContext(ProposalContext);
  if (!ctx) {
    throw new Error('useProposals must be used within a ProposalProvider');
  }
  return ctx;
}
