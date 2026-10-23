const assert = require("node:assert/strict");
const { after, test } = require("node:test");
const express = require("express");
const { createCorsMiddleware } = require("../src/middleware/cors");

const servers = [];

after(async () => {
  await Promise.all(
    servers.map((server) => new Promise((resolve) => server.close(resolve)))
  );
});

async function startServer(env) {
  const app = express();
  app.use(createCorsMiddleware(env));
  app.get("/resource", (_req, res) => res.json({ ok: true }));
  const server = app.listen(0);
  servers.push(server);
  await new Promise((resolve) => server.once("listening", resolve));
  return `http://127.0.0.1:${server.address().port}`;
}

test("restricted environments allow configured origins and preflight headers", async () => {
  const baseUrl = await startServer({
    NODE_ENV: "production",
    CORS_ALLOWED_ORIGINS: "https://app.example.com, https://admin.example.com",
  });
  const response = await fetch(`${baseUrl}/resource`, {
    method: "OPTIONS",
    headers: {
      Origin: "https://app.example.com",
      "Access-Control-Request-Method": "GET",
      "Access-Control-Request-Headers": "authorization,content-type",
    },
  });

  assert.equal(response.status, 204);
  assert.equal(
    response.headers.get("access-control-allow-origin"),
    "https://app.example.com"
  );
  assert.match(
    response.headers.get("access-control-allow-headers"),
    /authorization/i
  );
});

test("restricted environments do not approve unlisted origins", async () => {
  const baseUrl = await startServer({
    NODE_ENV: "production",
    CORS_ALLOWED_ORIGINS: "https://app.example.com",
  });
  const response = await fetch(`${baseUrl}/resource`, {
    headers: { Origin: "https://evil.example" },
  });

  assert.equal(response.status, 200);
  assert.equal(response.headers.get("access-control-allow-origin"), null);
});

test("development allows browser origins without an allowlist", async () => {
  const baseUrl = await startServer({ NODE_ENV: "development" });
  const response = await fetch(`${baseUrl}/resource`, {
    headers: { Origin: "http://localhost:5173" },
  });

  assert.equal(
    response.headers.get("access-control-allow-origin"),
    "http://localhost:5173"
  );
});