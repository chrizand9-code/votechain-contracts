import { beforeEach, describe, expect, it, vi } from "vitest";
import express from "express";
import request from "supertest";

const redisState = vi.hoisted(() => ({
  client: {
    isOpen: true,
    incr: vi.fn(),
    expire: vi.fn(),
  },
}));

vi.mock("../src/middleware/redisCache", () => ({
  getRedis: () => redisState.client,
}));

import csvRoutes, { capCsvRows, sendCsv } from "../src/routes/csv";

describe("CSV export limits", () => {
  beforeEach(() => {
    redisState.client.incr.mockReset().mockResolvedValue(1);
    redisState.client.expire.mockReset().mockResolvedValue(true);
  });

  it("caps rows at 10,000 and reports truncation headers", async () => {
    const rows = Array.from({ length: 10_001 }, (_, index) => index);
    const result = await capCsvRows(rows);

    expect(result.rows).toHaveLength(10_000);
    expect(result.count).toBe(10_000);
    expect(result.truncated).toBe(true);

    const app = express();
    app.get("/csv", async (_req, res) => {
      await sendCsv(res, "test.csv", "row", rows, String);
    });
    const response = await request(app).get("/csv");

    expect(response.status).toBe(200);
    expect(response.headers["x-record-count"]).toBe("10000");
    expect(response.headers["x-truncated"]).toBe("true");
    expect(response.text.split("\r\n")).toHaveLength(10_002);
  });

  it("limits CSV requests to five per minute for each API key", async () => {
    redisState.client.incr.mockResolvedValueOnce(5).mockResolvedValueOnce(6);
    const app = express();
    app.use((req, _res, next) => {
      req.apiKeyMeta = {
        id: "key-1",
        owner: "owner",
        label: "test",
        createdAt: new Date(0).toISOString(),
        status: "active",
        keyPrefix: "test-key",
      };
      next();
    });
    app.use("/api", csvRoutes);

    const allowed = await request(app).get("/api/proposals.csv");
    const limited = await request(app).get("/api/proposals.csv");

    expect(allowed.status).toBe(200);
    expect(limited.status).toBe(429);
    expect(redisState.client.incr).toHaveBeenNthCalledWith(
      1,
      "rate-limit:csv-export:key:key-1",
    );
  });
});
