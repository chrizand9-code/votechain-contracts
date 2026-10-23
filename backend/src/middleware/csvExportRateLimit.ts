import { NextFunction, Request, Response } from "express";
import { getRedis } from "./redisCache";
import { log } from "./requestTracing";

const WINDOW_SECONDS = 60;
const REQUEST_LIMIT = 5;

export async function csvExportRateLimit(
  req: Request,
  res: Response,
  next: NextFunction,
): Promise<void> {
  const redis = getRedis();
  if (!redis?.isOpen) {
    res.status(503).json({
      error: {
        code: "RATE_LIMIT_UNAVAILABLE",
        message: "CSV export rate limiting is temporarily unavailable.",
        details: [],
      },
    });
    return;
  }

  const clientId = req.apiKeyMeta
    ? `key:${req.apiKeyMeta.id}`
    : `ip:${req.ip}`;
  const key = `rate-limit:csv-export:${clientId}`;

  try {
    const count = await redis.incr(key);
    if (count === 1) {
      await redis.expire(key, WINDOW_SECONDS);
    }
    res.setHeader("X-RateLimit-Limit", REQUEST_LIMIT);
    res.setHeader("X-RateLimit-Remaining", Math.max(0, REQUEST_LIMIT - count));

    if (count > REQUEST_LIMIT) {
      res.status(429).json({
        error: {
          code: "RATE_LIMITED",
          message: "CSV export rate limit exceeded. Limit: 5 requests/min.",
          details: [],
        },
      });
      return;
    }
  } catch (error) {
    log("error", "CSV export rate limiter failed", { error: String(error) });
    res.status(503).json({
      error: {
        code: "RATE_LIMIT_UNAVAILABLE",
        message: "CSV export rate limiting is temporarily unavailable.",
        details: [],
      },
    });
    return;
  }

  next();
}
