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
 * Global error handling middleware for the VoteChain Express backend.
 *
 * All errors caught by Express (thrown inside route handlers or passed via
 * next(err)) are funnelled through globalErrorHandler, which normalises them
 * into the shared error envelope:
 *
 *   { "error": { "code": "...", "message": "...", "details": [] } }
 *
 * A companion notFoundHandler should be registered BEFORE globalErrorHandler
 * to catch unmatched routes.
 */

import { Request, Response, NextFunction } from "express";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** Shape of every error envelope this API returns. */
export interface ErrorEnvelope {
  error: {
    code: string;
    message: string;
    details: unknown[];
  };
}

/** Subset of properties we read from caught Error objects. */
interface HttpError extends Error {
  status?: number;
  statusCode?: number;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function buildEnvelope(code: string, message: string): ErrorEnvelope {
  return { error: { code, message, details: [] } };
}

function resolveStatus(err: HttpError): {
  httpStatus: number;
  code: string;
  message: string;
} {
  // JSON body parse errors from express.json()
  if (err instanceof SyntaxError) {
    return {
      httpStatus: 400,
      code: "BAD_REQUEST",
      message: err.message,
    };
  }

  const status = err.status ?? err.statusCode;

  if (status === 404 || err.name === "NotFoundError") {
    return { httpStatus: 404, code: "NOT_FOUND", message: err.message };
  }

  if (status === 403) {
    return { httpStatus: 403, code: "FORBIDDEN", message: err.message };
  }

  if (status === 401) {
    return { httpStatus: 401, code: "UNAUTHORIZED", message: err.message };
  }

  if (status === 400) {
    return { httpStatus: 400, code: "BAD_REQUEST", message: err.message };
  }

  if (status === 413) {
    return {
      httpStatus: 413,
      code: "PAYLOAD_TOO_LARGE",
      message: "Request body exceeds the configured size limit.",
    };
  }

  // Everything else is treated as an internal server error.
  return {
    httpStatus: 500,
    code: "INTERNAL_ERROR",
    // Never expose internal details to the client.
    message: "An unexpected error occurred. Please try again later.",
  };
}

// ---------------------------------------------------------------------------
// Middleware
// ---------------------------------------------------------------------------

/**
 * Global Express error-handling middleware (must be registered LAST, after all
 * routes and other middleware, and requires exactly 4 arguments so Express
 * recognises it as an error handler).
 */
// eslint-disable-next-line @typescript-eslint/no-unused-vars
export function globalErrorHandler(
  err: HttpError,
  req: Request,
  res: Response,
  // `next` is required by Express's error-handler signature even when unused.
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  next: NextFunction
): void {
  const { httpStatus, code, message } = resolveStatus(err);

  // For internal server errors, log the full stack server-side but return a
  // generic message so implementation details are never exposed to clients.
  if (httpStatus === 500) {
    console.error("[error] Unhandled exception:", err.stack ?? err.message);
  }

  res.status(httpStatus).json(buildEnvelope(code, message));
}

/**
 * Catch-all for routes that did not match any registered handler.
 * Register this BEFORE globalErrorHandler but AFTER all real routes.
 */
export function notFoundHandler(req: Request, res: Response): void {
  res
    .status(404)
    .json(
      buildEnvelope(
        "NOT_FOUND",
        `Route ${req.method} ${req.path} not found`
      )
    );
}
