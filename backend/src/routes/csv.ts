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
 * CSV export routes for proposal history.
 *
 * GET /proposals/:id/votes.csv  — streams all votes for a single proposal
 * GET /proposals.csv            — streams summary data for all proposals
 *
 * Both endpoints are RFC 4180 compliant, UTF-8 encoded, and stream the
 * response rather than buffering it in memory.
 *
 * Replace the TODO stubs with real DB / indexer queries once the data layer
 * is wired up.
 */

import { Router, Response } from "express";
import { PassThrough } from "stream";
import { csvExportRateLimit } from "../middleware/csvExportRateLimit";

const router = Router();
const MAX_CSV_EXPORT_ROWS = 10_000;

router.use(csvExportRateLimit);

interface CappedRows<T> {
  rows: T[];
  count: number;
  truncated: boolean;
}

export async function capCsvRows<T>(
  source: Iterable<T> | AsyncIterable<T>,
  maxRows = MAX_CSV_EXPORT_ROWS,
): Promise<CappedRows<T>> {
  const iterator: AsyncIterator<T> =
    Symbol.asyncIterator in Object(source)
      ? (source as AsyncIterable<T>)[Symbol.asyncIterator]()
      : (async function* () {
          yield* source as Iterable<T>;
        })();
  const rows: T[] = [];

  while (rows.length < maxRows) {
    const next = await iterator.next();
    if (next.done) {
      return { rows, count: rows.length, truncated: false };
    }
    rows.push(next.value);
  }

  const next = await iterator.next();
  if (!next.done) {
    await iterator.return?.();
  }

  return { rows, count: rows.length, truncated: !next.done };
}

export async function sendCsv<T>(
  res: Response,
  filename: string,
  columns: string,
  rows: Iterable<T> | AsyncIterable<T>,
  formatRow: (row: T) => string,
): Promise<void> {
  const result = await capCsvRows(rows);
  res.setHeader("X-Record-Count", result.count);
  if (result.truncated) {
    res.setHeader("X-Truncated", "true");
  }
  res.setHeader("Content-Type", "text/csv; charset=utf-8");
  res.setHeader("Content-Disposition", `attachment; filename="${filename}"`);

  const stream = new PassThrough();
  stream.pipe(res);
  stream.write(`${columns}\r\n`);
  for (const row of result.rows) {
    stream.write(`${formatRow(row)}\r\n`);
  }
  stream.end();
}

// ---------------------------------------------------------------------------
// GET /proposals/:id/votes.csv
// ---------------------------------------------------------------------------

/**
 * Downloads a CSV of every vote cast on a given proposal.
 *
 * Headers: proposal_id, voter, vote, weight, timestamp
 */
router.get("/proposals/:id/votes.csv", async (req, res) => {
  const { id } = req.params;

  // TODO: replace the empty source with a database cursor, e.g.:
  //   const rows = await db.query(
  //     `SELECT proposal_id, voter, vote, weight, timestamp
  //      FROM votes WHERE proposal_id = $1 ORDER BY timestamp ASC LIMIT 10001`,
  //     [id],
  //   );
  await sendCsv(
    res,
    `proposal-${id}-votes.csv`,
    "proposal_id,voter,vote,weight,timestamp",
    [],
    (row: { proposal_id: string; voter: string; vote: string; weight: number; timestamp: string }) =>
      `${row.proposal_id},${row.voter},${row.vote},${row.weight},${row.timestamp}`,
  );
});

// ---------------------------------------------------------------------------
// GET /proposals.csv
// ---------------------------------------------------------------------------

/**
 * Downloads a CSV summary of all proposals.
 *
 * Headers: id, title, state, proposer, votes_yes, votes_no, votes_abstain,
 *          quorum, start_time, end_time
 */
router.get("/proposals.csv", async (_req, res) => {
  // TODO: replace the empty source with a database cursor, e.g.:
  //   const rows = await db.query(
  //     `SELECT id, title, state, proposer, votes_yes, votes_no, votes_abstain,
  //             quorum, start_time, end_time
  //      FROM proposals ORDER BY id ASC LIMIT 10001`,
  //   );
  await sendCsv(
    res,
    "proposals.csv",
    "id,title,state,proposer,votes_yes,votes_no,votes_abstain,quorum,start_time,end_time",
    [],
    (row: {
      id: string;
      title: string;
      state: string;
      proposer: string;
      votes_yes: number;
      votes_no: number;
      votes_abstain: number;
      quorum: number;
      start_time: string;
      end_time: string;
    }) =>
      `${row.id},"${row.title.replace(/"/g, '""')}",${row.state},${row.proposer},` +
      `${row.votes_yes},${row.votes_no},${row.votes_abstain},${row.quorum},` +
      `${row.start_time},${row.end_time}`,
  );
});

export default router;
