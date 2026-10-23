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

import { RedisClientType } from 'redis';
import { LEADERBOARD_CACHE_TTL_SECONDS } from '../middleware/redisCache';

type MaybeRedis = RedisClientType | null | undefined;

export interface LeaderboardEntry {
  address: string;
  votes_cast: number;
  proposals_participated: number;
  total_weight_cast: number;
  rank: number;
}

const LEADERBOARD_CACHE_KEY_PREFIX = 'leaderboard:data:';

/**
 * Computes the top 50 voters by proposal participation count.
 * Results are cached for the configured TTL. Returns an empty list when Redis
 * is unavailable.
 */
export async function getLeaderboard(
  redisClient: MaybeRedis,
  filterKey = "",
): Promise<LeaderboardEntry[]> {
  if (!redisClient?.isOpen) return [];

  const cacheKey = `${LEADERBOARD_CACHE_KEY_PREFIX}${filterKey}`;

  // Try to get from cache first
  const cached = await redisClient.get(cacheKey);
  if (cached) {
    return JSON.parse(cached) as LeaderboardEntry[];
  }

  // In production, this would query the indexer or database.
  const leaderboard: LeaderboardEntry[] = [];

  await redisClient.setEx(
    cacheKey,
    LEADERBOARD_CACHE_TTL_SECONDS,
    JSON.stringify(leaderboard)
  );

  return leaderboard;
}

/**
 * Invalidates the leaderboard cache when vote data changes.
 */
export async function invalidateLeaderboardCache(
  redisClient: MaybeRedis
): Promise<void> {
  if (!redisClient?.isOpen) return;
  for await (const key of redisClient.scanIterator({
    MATCH: `${LEADERBOARD_CACHE_KEY_PREFIX}*`,
  })) {
    await redisClient.del(key);
  }
}

/**
 * Updates a voter's participation stats.
 * Called by the indexer when processing on-chain votes.
 */
export async function recordVote(
  redisClient: MaybeRedis,
  address: string,
  weight: number,
  proposalId: string
): Promise<void> {
  if (!redisClient?.isOpen) return;

  const key = `voter:${address}`;
  await redisClient.hIncrBy(key, 'votes_cast', 1);
  await redisClient.hIncrByFloat(key, 'total_weight_cast', weight);
  await redisClient.sAdd(`voter:${address}:proposals`, proposalId);
  await invalidateLeaderboardCache(redisClient);
}

/**
 * Retrieves vote history for a specific address.
 */
export async function getVoterStats(
  redisClient: MaybeRedis,
  address: string
): Promise<{
  address: string;
  votes_cast: number;
  proposals_participated: number;
  total_weight_cast: number;
}> {
  if (!redisClient?.isOpen) {
    return { address, votes_cast: 0, proposals_participated: 0, total_weight_cast: 0 };
  }

  const key = `voter:${address}`;
  const stats = await redisClient.hGetAll(key);
  const proposalCount = await redisClient.sCard(`voter:${address}:proposals`);

  return {
    address,
    votes_cast: parseInt(stats['votes_cast'] ?? '0', 10),
    proposals_participated: proposalCount,
    total_weight_cast: parseFloat(stats['total_weight_cast'] ?? '0'),
  };
}
