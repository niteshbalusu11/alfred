# Engineering Standards (Scalability + Security)

- Last Updated: 2026-02-14
- Priority: Mandatory for all issue execution

## 1) Post-Issue Deep Review (Required)

After implementing any issue, complete all of the following before handoff:

1. Run `just backend-deep-review` for backend changes.
2. Perform a manual security review focused on:
   1. auth/authz and token handling
   2. input validation and error paths
   3. secret/log redaction and data minimization
   4. privacy boundary regressions
3. Perform a manual refactor/scalability review focused on:
   1. layer boundaries and module ownership
   2. duplication and maintainability risks
   3. high-load behavior and growth constraints
4. Perform a manual bug check focused on:
   1. edge cases and invalid input behavior
   2. regression risk in changed paths
   3. deterministic failure handling
5. Post findings in the issue comment:
   1. list concrete findings or state `no findings`
   2. include unresolved risks and follow-ups
6. Before merge, add the structured AI review report in PR/issue using:
   1. `docs/ai-review-template.md`

## 2) Backend Layering Rules (Required)

Use strict separation of concerns:

1. Database/repository code:
   1. Must live under `backend/crates/shared/src/repos`
   2. `sqlx` queries must not appear in HTTP handler modules
2. HTTP API code:
   1. Must live under `backend/crates/api-server/src/http/*` modules
   2. Should handle request/response mapping and auth middleware only
3. Startup/bootstrap code:
   1. `main.rs` should wire config, infra, and router construction only
4. Worker runtime:
   1. Worker entrypoint orchestrates ticking and lifecycle
   2. DB access goes through repository layer

## 3) Maintainability and File Decomposition (Required)

To prevent codebase entropy and scaling bottlenecks:

1. Keep modules small and single-purpose; avoid "god files".
2. Handwritten source files should target `<= 300` lines.
3. Any handwritten source file above `500` lines must be actively decomposed when modified, unless there is a documented blocker.
4. Do not add substantial new logic to existing files already above `500` lines without first extracting modules.
5. Allowed size exceptions:
   1. generated files
   2. migration SQL
   3. test fixture data files
6. Each backend-impacting PR must document structural changes:
   1. modules extracted/added
   2. deferred decomposition follow-ups (with issue links) if any

## 4) Scalability and Reliability Requirements

All new code should be designed for growth:

1. Keep APIs deterministic and backward-compatible with OpenAPI.
2. Avoid hidden cross-layer coupling.
3. Keep writes idempotent where retries are expected.
4. Add indexes for query paths that become hot.
5. Keep error handling explicit and observable.

## 5) Security Baseline

1. No plaintext persistence of secrets/tokens.
2. No secret values in logs, traces, or errors.
3. Keep least-privilege behavior by default.
4. Fail closed for invalid auth or invalid security state.
