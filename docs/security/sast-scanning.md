# SAST Scanning

VoteChain runs static application security testing (SAST) across both languages in the
repository, using the tool best suited to each:

## TypeScript / JavaScript — CodeQL

`.github/workflows/codeql.yml` runs GitHub CodeQL's `javascript-typescript` analysis with the
`security-extended` query pack. The job declares no `paths`/`paths-ignore` filter, so it analyses
every TypeScript/JavaScript source tree in the repository, not just `backend/` and `frontend/` —
this also covers `sdk/` (`sdk/src/*.ts`) and `bots/governance-alerts/index.ts`, both of which exist
in-tree and previously weren't called out here.

- **Triggers:** every push to `main`, every pull request targeting `main`, and a weekly scheduled
  run (`0 8 * * 1`, Mondays 08:00 UTC) to catch newly published CodeQL queries against unchanged
  code.
- **Results:** SARIF output is uploaded automatically by `github/codeql-action/analyze` and appears
  under the repository's **Security → Code scanning alerts** tab.
- **PR gate:** the `enforce-severity-gate` job queries the code scanning alerts API for the PR's
  ref after analysis completes and fails the workflow if any **open alert with `critical` or
  `high` security severity** exists. This is a required status check, so such PRs cannot merge
  until the finding is fixed or dismissed.

## Rust — cargo-audit + Clippy (CodeQL alternative)

CodeQL's Rust support does not have a stable, general-purpose query pack (unlike its
JavaScript/TypeScript, Python, Go, Java, C/C++, C#, and Ruby support), so running it against
`contracts/`, `indexer/`, and `api/` would give a false sense of coverage. Instead, Rust SAST is
covered by two tools already wired into CI:

| Concern | Tool | Where |
|---------|------|-------|
| Known-vulnerable dependencies (CVE/RUSTSEC advisories) | `cargo audit --deny warnings` | `.github/workflows/audit.yml` (dedicated job) and `ci.yml`'s `security-audit` job, both on every push/PR, plus scheduled runs |
| Common bug patterns, unsafe idioms, correctness lints | `cargo clippy --all-targets -- -D warnings` | `.github/workflows/ci.yml`'s `lint` job, on every push/PR |

Both are required status checks — a warning from either tool fails the workflow, which has the
same blocking effect on PRs that CodeQL's severity gate has for the TypeScript side.

If CodeQL later ships first-class Rust support, this document should be revisited and the
`analyze` job's `languages` matrix expanded to include `rust` again.

## Existing findings triage (issue #310 acceptance criterion)

As of this writing, triaging **existing** open code-scanning alerts in the GitHub Security tab
requires `security_events`/code-scanning read access on the repository, which is outside what a
contributor PR can grant itself (the alert API returns `403` for accounts without that repo-level
permission — confirmed while preparing this change). A maintainer with admin or security-manager
access on `veracindarella/votechain-contracts` should run:

```
gh api repos/veracindarella/votechain-contracts/code-scanning/alerts --jq \
  '.[] | select(.state=="open") | {number, severity: .rule.security_severity_level, rule: .rule.id, path: .most_recent_instance.location.path}'
```

and either fix or dismiss (with a documented reason, e.g. "false positive" or "won't fix: accepted
risk") every alert with `security_severity_level` of `critical` or `high`, since those already
block merges via `enforce-severity-gate`. This section exists so that triage step isn't silently
skipped — it should be marked done here once a maintainer completes it, or converted into a
dedicated `SEC-0NN` finding if any alert turns out to be a real, unresolved vulnerability.

## Scan Frequency Summary

| Language | Trigger | Blocks PR on critical/high? |
|----------|---------|------------------------------|
| TypeScript/JavaScript | push to `main`, PR to `main`, weekly (Mon 08:00 UTC) | Yes — `enforce-severity-gate` |
| Rust | push to `main`, PR to `main` (via `ci.yml`/`audit.yml`) | Yes — any `clippy`/`cargo audit` failure blocks the required checks |
