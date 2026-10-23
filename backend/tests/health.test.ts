import { beforeEach, describe, expect, it, vi } from "vitest";
import express from "express";
import request from "supertest";

const healthMocks = vi.hoisted(() => ({
  dbQuery: vi.fn(),
  redisPing: vi.fn(),
}));

vi.mock("pg", () => ({
  Pool: vi.fn().mockImplementation(() => ({
    query: healthMocks.dbQuery,
  })),
}));

vi.mock("../src/middleware/redisCache", () => ({
  getRedis: () => ({ isOpen: true, ping: healthMocks.redisPing }),
}));

import healthRoutes from "../src/routes/health";

const app = express().use(healthRoutes);

describe("dependency health checks", () => {
  beforeEach(() => {
    vi.stubEnv("DATABASE_URL", "postgres://localhost/test");
    vi.stubEnv("STELLAR_RPC_URL", "http://stellar-rpc.test");
    healthMocks.dbQuery.mockReset().mockResolvedValue({ rows: [{ "?column?": 1 }] });
    healthMocks.redisPing.mockReset().mockResolvedValue("PONG");
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({ result: { status: "healthy" } }),
      }),
    );
  });

  it("returns component health and HTTP 200 when every dependency is healthy", async () => {
    const response = await request(app).get("/health");

    expect(response.status).toBe(200);
    expect(response.body.components).toEqual({
      db: "healthy",
      redis: "healthy",
      stellar_rpc: "healthy",
    });
  });

  it("returns HTTP 503 when a dependency check fails", async () => {
    healthMocks.redisPing.mockRejectedValue(new Error("connection refused"));

    const response = await request(app).get("/ready");

    expect(response.status).toBe(503);
    expect(response.body.components.redis).toBe("degraded");
  });

  it("keeps liveness independent of external dependencies", async () => {
    const response = await request(app).get("/live");

    expect(response.status).toBe(200);
    expect(response.body.status).toBe("ok");
  });
});
