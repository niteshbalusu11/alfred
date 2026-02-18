# Alfred Agent Instructions

This file is intentionally at repository root so coding agents can auto-discover it.

## Start Here

1. Read `docs/product-context.md` first.
2. Read `agent/start.md` before making changes.
3. If running with minimal context, read `agent/start.empty` first and then `agent/start.md`.
4. Read `docs/engineering-standards.md` for mandatory security/scalability rules.
5. For iOS/front-end issues, read `docs/ui-spec.md` before making changes.
6. Use the `Justfile` at repo root for all common workflows.
7. Work from GitHub issues first, then keep the Phase I board aligned.

## Authentication Direction (Important)

1. Auth direction is Clerk-based (migration epic: GitHub issue `#52`).
2. New auth work must align with issues `#53`, `#54`, and `#56`.
3. Breaking changes are acceptable for auth migration at this phase.
4. Custom `/v1/auth/ios/session*` paths should be removed or hard-disabled once Clerk path is complete.

## LLM Backend Direction (Important)

1. Backend assistant direction is LLM-first with OpenRouter provider routing (GitHub issues `#91` through `#103`).
2. New AI backend work must use label `ai-backend` and align to the execution tracker issue `#103`.
3. Legacy rule-based assistant logic was removed (`#91`) and must not be reintroduced.
4. Do not remove core infrastructure required by LLM workflows:
   1. OAuth/connector lifecycle
   2. enclave/attestation token path
   3. worker lease/retry/idempotency engine
   4. push pipeline
   5. privacy + audit controls

## Assistant Routing Direction (Important)

1. Semantic planner migration tracker is GitHub issue `#180` and is implemented.
2. Assistant routing must remain planner-driven (schema-constrained), not keyword-driven.
3. Current policy scope is English-first routing; non-English queries must clarify safely unless explicitly expanded.
4. Preserve deterministic fallback and clarification behavior when planner confidence/contract validation fails.

## Content Blindness Migration (Important)

1. Privacy-boundary tracker for server-blind message content is GitHub issue `#146` (implemented through `#149`).
2. Delivery phases `#147`, `#148`, and `#149` are complete; keep invariants enforced in new work.
3. Use labels `phase-1`, `P0`, `backend`, and `content-blindness` for this migration line.
4. Target privacy boundary:
   1. Metadata visibility on server is acceptable.
   2. Message content must be encrypted client-side and decrypted only inside an attested enclave path.
5. Breaking protocol and contract changes are explicitly allowed pre-launch; optimize for clean boundaries rather than compatibility.
6. Do not add new plaintext message-content logging or persistence in server-control-plane components.

## Required Workflow

1. Run `just check-tools`.
2. For backend-impacting work, run `just backend-tests`.
3. Run `just ios-build`.
4. Make scoped changes.
5. Re-run relevant checks before finishing:
   1. `just ios-build`
   2. `just backend-tests` when backend behavior changes
   3. `just backend-verify` when backend behavior changes
   4. `just backend-eval` when AI backend prompt/contract/safety behavior changes
   5. `just ios-test` when iOS core logic changed

## Local Infrastructure (Postgres for Backend Work)

Use Docker Compose at repo root when backend work needs a local DB.

1. Run `just check-infra-tools`.
2. Start DB: `just infra-up`.
3. Apply migrations: `just backend-migrate`.
4. Stop DB only: `just infra-stop`.
5. Stop and delete DB volumes: `just infra-down`.

Default local DB:

1. Host: `127.0.0.1`
2. Port: `5432`
3. Database: `alfred`
4. User: `postgres`
5. Password: `postgres`
6. `DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/alfred`

One-shot backend testing workflow:

1. `just backend-tests` (runs infra checks, starts DB/Redis, applies migrations, runs backend tests + eval, then stops infra)

## Backend Quality Gate (Non-Negotiable)

For backend code changes, task is done only when all pass:

1. `just backend-fmt`
2. `just backend-clippy`
3. `just backend-tests`
4. `just backend-build`

Notes:

1. `cargo fmt` only formats code.
2. `cargo fmt` does not ensure clippy passes.
3. Preferred command: `just backend-verify`
4. Preferred local backend test workflow: `just backend-tests`

## Deep Review Gate (Non-Negotiable)

After every issue with backend impact, complete this before handoff:

1. Run `just backend-deep-review`.
2. Perform an AI deep code review for:
   1. Security bugs and privacy boundary regressions.
   2. Bug check (logic, edge cases, regressions, error handling).
   3. Refactoring/scalability risks and maintainability issues.
3. Post findings to the issue:
   1. List concrete findings or explicitly state `no findings`.
   2. Include any follow-up tasks.
4. Before merge, include the AI review report in the PR using:
   1. `docs/ai-review-template.md`

Required architecture boundary:

1. DB repository code must stay in `backend/crates/shared/src/repos`.
2. HTTP routing/handlers/middleware must stay in `backend/crates/api-server/src/http/*` modules.
3. `main.rs` files should remain startup/bootstrap only.

## Code Structure and File Size Policy (Non-Negotiable)

Code must remain modular by default. Do not keep adding logic to a single large file.

1. Keep one clear responsibility per file/module.
2. For handwritten source files, target `<= 300` lines.
3. If a file grows beyond `500` lines, split it in the same issue unless there is a documented blocker.
4. Do not increase line count in an already-large file (`> 500` lines) without first extracting modules/helpers.
5. Allowed exceptions:
   1. generated code
   2. migration SQL
   3. test fixture data files
6. Refactor trigger:
   1. If you touch a large file, include at least one modularization step in that same change when practical.
7. PR/issue notes must call out structural changes:
   1. list which modules were created/extracted
   2. mention any deferred split with a follow-up issue number

## Frontend Test Policy

1. Add/update Swift tests for core logic changes.
2. UI tests are optional for now for UI-only changes.
3. Always run `just ios-build`; run `just ios-test` for core logic test changes.

## Frontend UI Rules (Non-Negotiable)

1. `docs/ui-spec.md` is the front-end source of truth for Phase I UI/UX.
2. SwiftUI must stay modular and reusable; avoid monolithic screens.
3. Target `<= 300` lines for handwritten Swift files and treat `500` lines as hard ceiling unless blocked.
4. If touching a large front-end file, extract components/helpers in the same issue when practical.
5. For front-end implementation, use relevant SwiftUI skills when applicable:
   1. `swiftui-ui-patterns`
   2. `swiftui-view-refactor`
   3. `swiftui-performance-audit`
   4. `swift-concurrency-expert` when async behavior changes
6. Visual style baseline is the monochrome cartoony theme in `docs/ui-spec.md`:
   1. four-tone grayscale palette (`ink`, `charcoal`, `smoke`, `paper`)
   2. thick outlines + hard shadows for depth
   3. no ad hoc hue accents unless explicitly approved

## Planning and Issue Source of Truth

1. Execution queue: GitHub issues in `niteshbalusu11/alfred` with labels:
   1. `phase-1`
   2. `P0` or `P1`
   3. `ai-backend` for LLM backend migration work
2. Planning board: `docs/phase1-master-todo.md`
3. Rule:
   1. If a GitHub issue and board item conflict, treat GitHub issue as immediate execution source and update docs in the same change.

## GitHub Issue Workflow (Required)

1. Pick next issue in priority order:
   1. `phase-1 + P0` first
   2. Then `phase-1 + P1`
   3. Within same priority, pick lowest issue number unless blocked by dependencies
2. Before coding:
   1. Sync current branch/worktree with remote:
      1. `git fetch origin`
      2. `git pull --ff-only` (when current branch tracks a remote)
   2. Confirm issue acceptance criteria and dependencies
   3. Update issue comment/status to indicate active work
3. Branching:
   1. Use branch name prefix `codex/`
   2. Recommended format: `codex/issue-<number>-<short-slug>`
   3. Create the branch from latest `origin/master`:
      1. `git checkout -b codex/issue-<number>-<short-slug> origin/master`
4. During implementation:
   1. Keep scope strictly to issue acceptance criteria
   2. If scope must expand, document reason in issue comment first
5. Before handoff:
   1. Run required checks from this file
   2. Summarize what passed/failed
   3. Update issue with completion notes and any follow-ups
6. If blocked:
   1. Add explicit blocker comment to issue
   2. Move to next unblocked highest-priority issue
7. Use standard issue update format:
   1. `docs/issue-update-template.md`

## PR and Merge Lifecycle (Required)

1. Create PR from working branch to `master`.
2. Wait for GitHub Actions checks to pass:
   1. `Backend Checks`
   2. `iOS Build`
3. Complete AI review report (security + bugs + scalability/cleanliness) before merge handoff.
4. Default behavior: hand off to maintainer for manual merge after required checks are green and AI review is documented.
5. Exception: an agent may merge when explicitly instructed by the user/automation owner, all required checks are green, and the AI review report is documented.
6. After merge (maintainer or approved agent), sync local environment:
   1. `just sync-master`
7. Start next task from updated `master`.

## Key Paths

1. iOS app: `alfred`
2. iOS API package: `alfred/Packages/AlfredAPIClient`
3. Backend workspace: `backend`
4. OpenAPI contract: `api/openapi.yaml`
5. DB migrations: `db/migrations`
6. Phase I board: `docs/phase1-master-todo.md`
7. Product context: `docs/product-context.md`

## Guardrails

1. Keep API/server/client contracts aligned.
2. Use migrations for any schema changes.
3. Do not weaken privacy constraints (token/data handling) without explicit approval.
4. Do not implement work outside an active GitHub issue unless explicitly requested.
5. For LLM features, enforce schema-validated outputs and deterministic safety fallback behavior.
