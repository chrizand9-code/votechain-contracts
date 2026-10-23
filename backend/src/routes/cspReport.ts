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
 * POST /csp-report
 *
 * Receives Content-Security-Policy violation reports forwarded by Nginx
 * (see frontend/deploy/nginx.conf and docs/security/SEC-012-content-security-policy.md).
 *
 * Browsers send these with either:
 *   - Content-Type: application/csp-report       (legacy `report-uri` format)
 *   - Content-Type: application/reports+json     (newer `report-to` format)
 *
 * Neither is parsed by the default express.json() middleware (which only
 * handles application/json), so this route carries its own JSON body
 * parser configured for those content types. Without it, every report
 * Nginx forwards here previously fell through to the 404 handler and was
 * silently dropped — meaning CSP violations (including real XSS attempts)
 * were never logged.
 */

import { Router, Request, Response } from "express";
import express from "express";

const router = Router();

const cspReportBodyParser = express.json({
  type: ["application/csp-report", "application/reports+json", "application/json"],
  limit: "16kb",
});

router.post(
  "/csp-report",
  cspReportBodyParser,
  (req: Request, res: Response) => {
    const body = req.body;

    // report-uri format: { "csp-report": { ... } }
    // report-to format:  [ { type: "csp-violation", body: { ... } }, ... ]
    const reports = Array.isArray(body)
      ? body
      : body && body["csp-report"]
        ? [body["csp-report"]]
        : body
          ? [body]
          : [];

    for (const report of reports) {
      console.warn("[csp-violation]", JSON.stringify(report));
    }

    // Browsers do not read the response body for report endpoints;
    // 204 No Content is the conventional response.
    res.status(204).end();
  }
);

export default router;
