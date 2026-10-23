import cors, { CorsOptions } from "cors";
import { RequestHandler } from "express";

const RESTRICTED_ENVIRONMENTS = new Set(["production", "staging", "mainnet"]);

export function createCorsMiddleware(
  env: NodeJS.ProcessEnv = process.env
): RequestHandler {
  const allowedOrigins = new Set(
    (env.CORS_ALLOWED_ORIGINS ?? "")
      .split(",")
      .map((origin) => origin.trim())
      .filter(Boolean)
  );
  const restricted =
    RESTRICTED_ENVIRONMENTS.has((env.NODE_ENV ?? "").toLowerCase()) ||
    RESTRICTED_ENVIRONMENTS.has((env.NETWORK ?? "").toLowerCase());

  const options: CorsOptions = {
    origin(origin, callback) {
      if (!restricted || !origin || allowedOrigins.has(origin)) {
        callback(null, true);
        return;
      }
      callback(null, false);
    },
    optionsSuccessStatus: 204,
  };

  return cors(options);
}