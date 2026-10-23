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

import { Router, Request, Response } from 'express';
import { getLeaderboard, getVoterStats } from '../services/leaderboardService';
import { cacheLeaderboardResponse, getRedis } from '../middleware/redisCache';

const router = Router();

/**
 * GET /leaderboard
 * Returns top 50 voters sorted by proposal participation count
 *
 * Response:
 * {
 *   "leaderboard": [
 *     {
 *       "rank": 1,
 *       "address": "G...",
 *       "votes_cast": 50,
 *       "proposals_participated": 15,
 *       "total_weight_cast": 1500.5
 *     },
 *     ...
 *   ]
 * }
 */
router.get('/', cacheLeaderboardResponse, async (req: Request, res: Response) => {
  try {
    const leaderboard = await getLeaderboard(getRedis(), req.originalUrl);

    // Add rank to each entry
    const ranked = leaderboard.map((entry, index) => ({
      ...entry,
      rank: index + 1,
    }));

    return res.json({
      leaderboard: ranked,
      count: ranked.length,
    });
  } catch (error) {
    console.error('[Leaderboard] Error fetching leaderboard:', error);
    return res.status(500).json({ error: 'Internal server error' });
  }
});

/**
 * GET /leaderboard/address/:address
 * Get vote statistics for a specific address
 *
 * Response:
 * {
 *   "address": "G...",
 *   "votes_cast": 25,
 *   "proposals_participated": 10,
 *   "total_weight_cast": 750.25
 * }
 */
router.get('/address/:address', async (req: Request, res: Response) => {
  try {
    const { address } = req.params;

    if (!address || typeof address !== 'string') {
      return res.status(400).json({ error: 'Invalid address' });
    }

    const stats = await getVoterStats(getRedis(), address);

    return res.json(stats);
  } catch (error) {
    console.error('[Leaderboard] Error fetching voter stats:', error);
    return res.status(500).json({ error: 'Internal server error' });
  }
});

export default router;
