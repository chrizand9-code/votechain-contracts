import { describe, expect, it } from "vitest";
import express from "express";
import request from "supertest";
import { globalErrorHandler } from "../src/middleware/errorHandler";
import { jsonBodyParser } from "../src/middleware/jsonBodyParser";

function createApp() {
  const app = express();
  app.use(jsonBodyParser("100kb"));
  app.post("/", (_req, res) => res.sendStatus(204));
  app.use(globalErrorHandler);
  return app;
}

describe("JSON request body size limit", () => {
  it("accepts a body within the configured limit", async () => {
    const response = await request(createApp())
      .post("/")
      .send({ value: "small" });

    expect(response.status).toBe(204);
  });

  it("rejects oversized JSON with HTTP 413", async () => {
    const response = await request(createApp())
      .post("/")
      .send({ value: "x".repeat(1024 * 1024) });

    expect(response.status).toBe(413);
    expect(response.body.error.code).toBe("PAYLOAD_TOO_LARGE");
  });
});
