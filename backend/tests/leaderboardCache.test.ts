import { beforeEach, describe, expect, it, vi } from "vitest";
import express from "express";
import request from "supertest";

const redisState = vi.hoisted(() => ({
  values: new Map<string, string>(),
  client: {
    isOpen: true,
    on: vi.fn().mockReturnThis(),
    connect: vi.fn().mockResolvedValue(undefined),
    get: vi.fn(),
    setEx: vi.fn(),
  },
}));

vi.mock("redis", () => ({
  createClient: () => redisState.client,
}));

vi.mock("../src/services/leaderboardService", () => ({
  getLeaderboard: vi.fn().mockResolvedValue([]),
  getVoterStats: vi.fn().mockResolvedValue({}),
}));

import { connectRedis } from "../src/middleware/redisCache";
import leaderboardRoutes from "../src/routes/leaderboard";
import { getLeaderboard } from "../src/services/leaderboardService";

const app = express().use("/api/leaderboard", leaderboardRoutes);

describe("leaderboard response cache", () => {
  beforeEach(async () => {
    redisState.values.clear();
    redisState.client.get.mockImplementation(
      async (key: string) => redisState.values.get(key) ?? null,
    );
    redisState.client.setEx.mockImplementation(
      async (key: string, _ttl: number, value: string) => {
        redisState.values.set(key, value);
        return "OK";
      },
    );
    await connectRedis("redis://test");
    vi.mocked(getLeaderboard).mockClear();
  });

  it("returns X-Cache MISS then HIT and caches filter-specific keys for 60 seconds", async () => {
    const first = await request(app).get("/api/leaderboard?state=active");
    const cached = await request(app).get("/api/leaderboard?state=active");
    const differentFilter = await request(app).get(
      "/api/leaderboard?state=closed",
    );

    expect(first.headers["x-cache"]).toBe("MISS");
    expect(cached.headers["x-cache"]).toBe("HIT");
    expect(differentFilter.headers["x-cache"]).toBe("MISS");
    expect(redisState.client.setEx).toHaveBeenCalledWith(
      "leaderboard:response:/api/leaderboard?state=active",
      60,
      expect.any(String),
    );
    expect(getLeaderboard).toHaveBeenCalledTimes(2);
  });
});
