// Copyright 2024 VoteChain Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this License except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

import React, { useCallback, useEffect, useMemo, useState } from "react";
import { getAccountExplorerUrl, truncateAddress } from "../utils/explorer";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface LeaderboardEntry {
  rank: number;
  address: string;
  votes_cast: number;
  proposals_participated: number;
  total_weight_cast: number;
}

interface LeaderboardResponse {
  leaderboard: LeaderboardEntry[];
  count: number;
}

type SortKey = "participation" | "weight";
type SortDir = "asc" | "desc";

const PAGE_SIZE = 20;

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export default function Leaderboard() {
  const [entries, setEntries] = useState<LeaderboardEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [sortKey, setSortKey] = useState<SortKey>("participation");
  const [sortDir, setSortDir] = useState<SortDir>("desc");
  const [page, setPage] = useState(0);

  // ── Fetch leaderboard data ───────────────────────────────────────────────

  const fetchLeaderboard = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await fetch("/api/leaderboard");
      if (!res.ok) {
        throw new Error(`HTTP ${res.status}: ${res.statusText}`);
      }
      const data: LeaderboardResponse = await res.json();
      setEntries(data.leaderboard ?? []);
    } catch (err: unknown) {
      setError(
        (err as { message?: string })?.message ?? "Failed to load leaderboard."
      );
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchLeaderboard();
  }, [fetchLeaderboard]);

  // ── Sorting ────────────────────────────────────────────────────────────────

  const sortedEntries = useMemo(() => {
    const sorted = [...entries].sort((a, b) => {
      let cmp: number;
      if (sortKey === "participation") {
        cmp = a.proposals_participated - b.proposals_participated;
      } else {
        cmp = a.total_weight_cast - b.total_weight_cast;
      }
      return sortDir === "desc" ? -cmp : cmp;
    });
    return sorted;
  }, [entries, sortKey, sortDir]);

  // ── Pagination ─────────────────────────────────────────────────────────────

  const totalPages = Math.max(1, Math.ceil(sortedEntries.length / PAGE_SIZE));
  const currentPage = Math.min(page, totalPages - 1);
  const pageEntries = sortedEntries.slice(
    currentPage * PAGE_SIZE,
    currentPage * PAGE_SIZE + PAGE_SIZE
  );

  function handleSort(key: SortKey) {
    if (sortKey === key) {
      setSortDir((d) => (d === "desc" ? "asc" : "desc"));
    } else {
      setSortKey(key);
      setSortDir("desc");
    }
    setPage(0);
  }

  function getSortIndicator(key: SortKey): string {
    if (sortKey !== key) return "↕";
    return sortDir === "desc" ? "↓" : "↑";
  }

  function fmtWeight(n: number): string {
    return n.toLocaleString(undefined, { maximumFractionDigits: 2 });
  }

  // ── Render ────────────────────────────────────────────────────────────────

  return (
    <section aria-labelledby="leaderboard-title" className="card">
      <h2 id="leaderboard-title">Voter Leaderboard</h2>
      <p>
        Top voters ranked by governance participation. Click a column header to sort.
      </p>

      {/* Loading */}
      {loading && (
        <p aria-busy="true" aria-live="polite">
          Loading leaderboard…
        </p>
      )}

      {/* Error */}
      {!loading && error && (
        <div role="alert" className="error-state">
          <p>{error}</p>
          <button type="button" onClick={fetchLeaderboard} className="filter-btn">
            Retry
          </button>
        </div>
      )}

      {/* Empty state */}
      {!loading && !error && entries.length === 0 && (
        <div role="status" aria-live="polite">
          <p>No voter data available yet.</p>
        </div>
      )}

      {/* Table */}
      {!loading && !error && entries.length > 0 && (
        <>
          <div className="table-wrapper" aria-live="polite">
            <table>
              <caption className="visually-hidden">
                Voter leaderboard sorted by {sortKey === "participation" ? "participation count" : "total vote weight"}, {sortDir === "desc" ? "descending" : "ascending"}
              </caption>
              <thead>
                <tr>
                  <th scope="col">Rank</th>
                  <th scope="col">Address</th>
                  <th scope="col">
                    <button
                      type="button"
                      onClick={() => handleSort("participation")}
                      aria-sort={
                        sortKey === "participation"
                          ? sortDir === "desc"
                            ? "descending"
                            : "ascending"
                          : "none"
                      }
                      style={{
                        background: "none",
                        border: "none",
                        cursor: "pointer",
                        fontWeight: "inherit",
                        fontSize: "inherit",
                        color: "inherit",
                      }}
                    >
                      Proposals Voted {getSortIndicator("participation")}
                    </button>
                  </th>
                  <th scope="col">
                    <button
                      type="button"
                      onClick={() => handleSort("weight")}
                      aria-sort={
                        sortKey === "weight"
                          ? sortDir === "desc"
                            ? "descending"
                            : "ascending"
                          : "none"
                      }
                      style={{
                        background: "none",
                        border: "none",
                        cursor: "pointer",
                        fontWeight: "inherit",
                        fontSize: "inherit",
                        color: "inherit",
                      }}
                    >
                      Total Vote Weight {getSortIndicator("weight")}
                    </button>
                  </th>
                </tr>
              </thead>
              <tbody>
                {pageEntries.map((entry) => (
                  <tr key={entry.address}>
                    <td>{entry.rank}</td>
                    <td>
                      <a
                        href={getAccountExplorerUrl(entry.address)}
                        target="_blank"
                        rel="noreferrer"
                        title={entry.address}
                        aria-label={`View ${entry.address} on Stellar Explorer`}
                      >
                        {truncateAddress(entry.address)}
                      </a>
                    </td>
                    <td>{entry.proposals_participated}</td>
                    <td>{fmtWeight(entry.total_weight_cast)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {/* Pagination */}
          {totalPages > 1 && (
            <nav aria-label="Leaderboard pagination" style={{ marginTop: 16, display: "flex", gap: 8, alignItems: "center" }}>
              <button
                type="button"
                onClick={() => setPage((p) => Math.max(0, p - 1))}
                disabled={currentPage === 0}
                aria-label="Previous page"
                aria-disabled={currentPage === 0}
              >
                ← Prev
              </button>
              <span aria-current="page">
                Page {currentPage + 1} of {totalPages}
              </span>
              <button
                type="button"
                onClick={() => setPage((p) => Math.min(totalPages - 1, p + 1))}
                disabled={currentPage >= totalPages - 1}
                aria-label="Next page"
                aria-disabled={currentPage >= totalPages - 1}
              >
                Next →
              </button>
            </nav>
          )}
        </>
      )}
    </section>
  );
}
