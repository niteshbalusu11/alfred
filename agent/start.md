# Alfred Agent Start Guide

## Purpose

This file is the operational and technical entry point for agents working in this repository.
Use it to understand the architecture quickly and run the system with consistent commands.

Read this first for product/business context:

`docs/product-context.md`

Read mandatory security/scalability rules:

`docs/engineering-standards.md`

For iOS/front-end issues, read UI source of truth:

`docs/ui-spec.md`

Use this template for required AI review reporting before merge:

`docs/ai-review-template.md`

Assistant v2 privacy/eval guardrails and verification checklist:

`docs/assistant-v2-privacy-eval.md`

## Project Summary

Alfred is a privacy-first iOS assistant product with a hosted backend.
Current v1 scope:

1. Google Calendar meeting reminders.
2. Daily morning brief (LLM-generated summary path).
3. Urgent Gmail alerting (LLM prioritization path).
4. Natural-language assistant query endpoint (for example, \"What meetings do I have today?\").
5. Active migration: replace hardcoded proactive jobs with client-defined periodic automation jobs (tracker `#208`).

The project intentionally avoids smart-home control in v1 to reduce reliability and liability risk.

## Repository Map

1. iOS app project:
   1. `alfred`
2. Local Swift package used by iOS client code:
   1. `alfred/Packages/AlfredAPIClient`
3. Backend Rust workspace:
   1. `backend`
4. API contract:
   1. `api/openapi.yaml`
5. DB migrations:
   1. `db/migrations`
6. Product/architecture RFC:
   1. `docs/rfc-0001-alfred-ios-v1.md`
7. Phase I master board:
   1. `docs/phase1-master-todo.md`
8. Product context (canonical):
   1. `docs/product-context.md`
9. Logging conventions:
   1. `docs/logging.md`
10. UI/UX source of truth for iOS:
   1. `docs/ui-spec.md`

## Runtime Components

1. iOS App (`SwiftUI`):
   1. UX surface for authentication, connector setup, settings, and notifications.
   2. Assistant TTS path is local/on-device KittenTTS (ONNX) playback.
2. API Server (`Rust + axum`):
   1. Serves v1 REST endpoints aligned with OpenAPI.
3. Worker (`Rust + tokio`):
   1. Periodic processing loop for scheduled/proactive jobs.
4. Shared crate:
   1. Shared request/response models and basic runtime config.

## iOS Speech Architecture (2026-02-19)

1. User input transcription uses Apple's `Speech` framework (`VoiceLiveSpeechRecognizer`).
2. Assistant output speech uses local KittenTTS synthesis and waveform playback.
3. Do not reintroduce `AVSpeechSynthesizer` for assistant replies.
4. Keep model and voice-style assets in `alfred/alfred/Resources/KittenTTS` aligned with `KittenSpeechModelStore` and `KittenVoiceStyleStore`.

## Authentication Source Of Truth (2026-02-14)

1. Phase I auth direction is Clerk-based (epic `#52`).
2. Use Clerk token verification/mapping path for new auth work (`#53`).
3. Use Clerk iOS integration for app sign-in/token plumbing (`#54`).
4. Remove or hard-disable custom `/v1/auth/ios/session*` flow as part of Clerk completion (`#56`).
5. Breaking auth changes are acceptable during this migration.

## LLM Backend Source Of Truth (2026-02-18)

1. Backend assistant direction is LLM-first with OpenRouter provider routing.
2. Execution queue for this migration is GitHub issues `#91` through `#103`.
3. Use label `ai-backend` for all related backend issues.
4. Legacy rule-based assistant decision logic was removed (`#91`) and must not be reintroduced.
5. Semantic planner routing migration (`#180`) is implemented and is now the required decision path.
6. Current planner policy is English-first with deterministic clarification for unsupported language cases.
7. Keep privacy and reliability infrastructure intact:
   1. OAuth/connector lifecycle
   2. enclave/attestation token path
   3. worker lease/retry/idempotency engine
   4. push pipeline
   5. audit/privacy controls
8. Do not implement or expand backend English keyword routing lists (including temporal words like `today`/`tomorrow`) as primary intent/time logic.
9. Always provide planner prompts with current timestamp + timezone + prior context and let schema-constrained planner outputs drive routing/time windows.
10. Deterministic fallback is allowed only as a safety net and should rely on generic structural behavior, not hard-coded language phrase tables.

## Content Blindness Source Of Truth (2026-02-18)

1. Privacy-boundary tracker for server-blind message content is GitHub issue `#146` and is implemented through `#149`.
2. Execution phases:
   1. `#147` design and protocol boundary definition
   2. `#148` encrypted message transport and enclave-only plaintext handling
   3. `#149` validation, hardening, and privacy verification gates
3. Current boundary:
   1. Metadata can remain visible to server control plane.
   2. User/assistant message body content must remain ciphertext outside the enclave.
4. Breaking API/protocol changes remain acceptable pre-launch when preserving this boundary.
5. Required labels for this workstream: `phase-1`, `P0`, `backend`, `content-blindness`.

## Automation v2 Source Of Truth (2026-02-20)

1. Tracker issue: `#208` (required execution issues `#209`, `#210`, `#211`, `#212`, `#213`, `#214`).
2. Migration policy:
   1. no feature flags
   2. no backward compatibility with legacy proactive job behavior
   3. remove legacy hardcoded worker dispatch paths as part of this line
3. Architecture target:
   1. iOS defines periodic automation rules and submits encrypted prompt envelope material
   2. backend persists schedule metadata + sealed prompt ciphertext only
   3. worker scheduler claims due rules via lease-safe claiming and materializes `AUTOMATION_RUN` jobs with deterministic idempotency keys (`{rule_id}:{scheduled_for}`)
   4. enclave executes prompt workflow and returns encrypted notification artifacts (no host plaintext)
   5. backend push sender emits encrypted APNs payload with `mutable-content`
   6. iOS Notification Service Extension decrypts and rewrites visible notification content locally
4. Non-negotiable privacy constraints:
   1. host must not process or persist automation prompt/output plaintext
   2. logs/audit must remain metadata-only and redacted
   3. plaintext rendering happens on-device after NSE decrypt
5. Worker reliability constraints remain mandatory:
   1. lease ownership
   2. deterministic retry classification
   3. idempotent run materialization/execution
   4. dead-letter behavior and observability metrics

## GitHub Issue-Driven Execution

Primary execution queue is GitHub issues in:

`https://github.com/niteshbalusu11/alfred/issues`

Phase I labels currently in use:

1. Scope: `phase-1`
2. Priority: `P0`, `P1`
3. Domain tags: `backend`, `ios`, `security`, `database`, `oauth`, `tee`, `worker`, `notifications`, `privacy`, `observability`, `sre`, `qa`, `ai-backend`

### Issue Selection Algorithm (Required)

1. Select open issues with `phase-1`.
2. Prioritize:
   1. `P0` before `P1`
   2. Lowest issue number first unless blocked by explicit dependency
3. If blocked:
   1. Comment blocker details on issue
   2. Move to next unblocked issue

### Issue Execution Protocol

1. Sync current branch/worktree with remote before coding:
   1. `git fetch origin`
   2. `git pull --ff-only` (when current branch tracks a remote)
2. Read issue acceptance criteria before coding.
3. Create branch with prefix `codex/`:
   1. Example: `codex/issue-4-worker-leasing`
   2. Branch from latest `origin/master`:
      1. `git checkout -b codex/issue-4-worker-leasing origin/master`
4. Implement only acceptance-criteria scope.
5. Run validation commands from `Justfile`.
6. Update issue with:
   1. What changed
   2. Validation results
   3. Remaining follow-ups
7. Use standard comment formats from:
   1. `docs/issue-update-template.md`

### PR Lifecycle Protocol

1. Open PR targeting `master`.
2. Wait for CI checks:
   1. `.github/workflows/ci.yml` (`Backend Checks`, `iOS Build`)
3. Complete AI review report (security + bug check + scalability/code quality) before merge handoff.
4. Default behavior: hand off to maintainer for manual merge after checks pass and AI review is documented.
5. Exception: an agent may merge when explicitly instructed by the user/automation owner, all required checks are green, and AI review is documented.
6. Once merge is complete:
   1. run `just sync-master`
   2. pick next highest-priority unblocked issue

### Sync Rule: GitHub Issues vs Phase I Board

1. GitHub issue is the immediate execution source.
2. `docs/phase1-master-todo.md` is the planning/control board.
3. If either side changes, keep them aligned in the same work cycle.
4. Do not start untracked work; create/obtain an issue first.

## Security and Privacy Constraints

Agents must preserve these constraints while implementing features:

1. No plaintext long-term storage of connector data (tokens/data should be encrypted at rest).
2. Data minimization for v1:
   1. Persist only minimal state needed for scheduling, retries, and auditing.
3. API changes must remain aligned with:
   1. `api/openapi.yaml`
4. Schema changes must be migration-driven:
   1. Add/modify SQL files under `db/migrations`.
5. LLM outputs must be schema-validated before UI/push emission.
6. Prompt/context payloads and logs must remain redacted and privacy-safe.

## Justfile Command Reference

All operational commands are centralized in:

`Justfile`

Run from repository root:

```bash
cd .
```

Primary commands:

1. `just check-tools`
   1. Verifies local toolchain (`xcodebuild`, `cargo`, `swift`).
2. `just check-infra-tools`
   1. Verifies local infra tooling (`docker`, `docker compose`).
3. `just infra-up`
   1. Starts local Postgres from `docker-compose.yml`.
4. `just infra-stop`
   1. Stops Postgres without deleting volumes.
5. `just infra-down`
   1. Stops Postgres and removes volumes.
6. `just backend-migrate`
   1. Applies SQL migrations from `db/migrations`.
7. `just backend-migrate-check`
   1. Prints migration status for configured `DATABASE_URL`.
8. `just ios-open`
   1. Opens the Xcode project.
9. `just ios-build`
   1. Builds iOS app for simulator.
10. `just ios-test`
   1. Runs iOS tests on default simulator destination.
11. `just ios-package-build`
   1. Builds the local Swift package (`AlfredAPIClient`).
12. `just backend-check`
   1. Runs Rust compile checks.
13. `just backend-build`
   1. Builds Rust backend workspace.
14. `just backend-tests`
   1. One-shot local backend test workflow (infra checks + infra up + migrate + tests + mocked eval).
   2. Do not run `just infra-stop` automatically after verification; keep infra running unless the user explicitly asks to stop it.
15. `just backend-test`
   1. Runs backend unit/module tests (excludes `integration-tests` crate).
16. `just backend-integration-test`
   1. Runs integration test crate (`integration-tests`) with default local `DATABASE_URL` fallback.
17. `just backend-eval`
   1. Runs deterministic LLM eval/regression checks in mocked mode.
18. `just backend-eval-update`
   1. Intentionally refreshes mocked-mode eval goldens after reviewed behavior changes.
19. `just backend-eval-live`
   1. Runs optional live-provider LLM smoke checks (requires `OPENROUTER_*` env vars).
20. `just backend-fmt`
   1. Formats Rust code.
21. `just backend-clippy`
    1. Runs lint checks with warnings denied.
22. `just backend-verify`
    1. Runs backend completion gate: fmt + clippy + tests + build.
23. `just backend-security-audit`
    1. Runs dependency vulnerability audit (`cargo audit`).
24. `just backend-bug-check`
    1. Runs backend tests and fails on placeholder/debug macros.
25. `just backend-architecture-check`
    1. Enforces DB/HTTP layer boundaries for scalability.
26. `just backend-deep-review`
    1. Runs backend verify + security audit + bug check + architecture checks.
27. `just backend-api`
    1. Runs REST API server.
28. `just backend-worker`
    1. Runs background worker.
29. `just api` (pending issue `#48`)
    1. Planned alias for `just backend-api` once `.env` startup support lands.
30. `just worker` (pending issue `#48`)
    1. Planned alias for `just backend-worker` once `.env` startup support lands.
31. `just dev`
    1. Runs API server + worker together.
32. `just docs`
    1. Prints key project documentation paths.
33. `just sync-master`
    1. Fetches remote, checks out `master`, and fast-forward pulls latest.

## Test and Quality Policy (Strict)

### Backend (Rust)

When backend code changes, unit/integration tests are required.

Required completion checks:

1. `just backend-fmt`
2. `just backend-clippy`
3. `just backend-tests`
4. `just backend-build`

Important:

1. `cargo fmt` only formats code.
2. `cargo fmt` does **not** replace linting or testing.
3. Backend task is not done until linting, tests, and build all pass.
4. Preferred one-shot command:
   1. `just backend-verify`
5. Preferred local backend test workflow:
   1. `just backend-tests`

### Frontend (iOS)

1. If changing core logic (state management, API client logic, data transforms), add/update Swift tests.
2. For UI-only changes in current phase, UI tests are optional and not required by default.
3. Minimum iOS completion check:
   1. `just ios-build`
4. Run `just ios-test` when logic tests were added/changed or when explicitly requested.
5. Follow `docs/ui-spec.md` for navigation, theme, state UX, and screen responsibilities.
6. Keep SwiftUI files modular:
   1. Target `<= 800` lines for handwritten files.
   2. If a file exceeds `800` lines, split it unless blocked and explicitly documented.
7. Prefer reusable components and shared patterns over one-off large view files.
8. Use SwiftUI skills when relevant:
   1. `swiftui-ui-patterns`
   2. `swiftui-view-refactor`
   3. `swiftui-performance-audit`
   4. `swift-concurrency-expert` for async/concurrency changes

### Swift Concurrency Guardrails For iOS/Test Stability

1. Prefer storing simple UI sync/flags state as value types on `AppModel` instead of separate actor-isolated reference helpers.
2. Do not add `@MainActor ObservableObject` helper objects when the owning `AppModel` can hold the same state directly.
3. Be strict about `Task` lifetime in tests and model helpers; cancel or scope tasks so teardown does not deallocate active actor-bound tasks.
4. Any crash signature involving `swift_task_deinitOnExecutorMainActorBackDeploy` or `TaskLocal::StopLookupScope` is a blocker and must be fixed before merge.
5. After concurrency-related changes, run:
   1. `just ios-test`
   2. targeted tests for model construction/deallocation paths

## Standard Agent Workflow

Use this sequence for most engineering tasks:

1. `just check-tools`
2. For backend-impacting work, run `just backend-tests`.
3. `just backend-check`
4. `just ios-package-build`
5. `just ios-build`
6. Implement change.
7. Re-run:
   1. `just ios-build`
8. If backend behavior changed, also run:
   1. `just backend-tests`
9. If backend behavior changed, also run:
   1. `just backend-deep-review`
10. If AI backend prompt/contract/safety behavior changed, also run:
   1. `just backend-eval`
11. If frontend core logic changed, also run:
   1. `just ios-test`
12. If API contract changed:
   1. Update `api/openapi.yaml`
   2. Ensure model updates in shared/server/client code.
13. If persistence changed:
   1. Add a new migration under `db/migrations`.
14. If issue state changed:
    1. Update GitHub issue comments/checklist
    2. Keep `docs/phase1-master-todo.md` status consistent where relevant
15. Before PR merge (mandatory for backend-impacting issues):
    1. Produce AI review report (security audit + bug check + scalability/cleanliness review)
    2. Use `docs/ai-review-template.md`
    3. Default: hand off to maintainer for manual merge after report is documented in issue/PR
    4. Exception: merge is allowed for agents only when explicitly requested by the user/automation owner and required checks are green

## Mandatory Scalability Boundaries

1. All database/repository code must live in `backend/crates/shared/src/repos`.
2. All API HTTP handlers/middleware/routing must live in `backend/crates/api-server/src/http/*` modules.
3. Keep `main.rs` limited to startup wiring/config/bootstrap.

## Mandatory Code Decomposition Policy

Avoid monolithic files. New work should keep files focused and easy to review.

1. Prefer one responsibility per file/module.
2. For handwritten source files, keep a target size of `<= 800` lines.
3. When a file exceeds `800` lines, split it into submodules in the same issue unless blocked.
4. If touching a file already over `800` lines, do not add net-new complexity without extracting logic first.
5. Exceptions are limited to:
   1. generated files
   2. migration SQL
   3. test fixture data files
6. During issue updates and PR summaries:
   1. explicitly list newly extracted modules
   2. include follow-up issue references for any deferred decomposition

## Current Known State (Post Semantic Planner Migration)

1. API server and worker are live with OAuth, preferences, privacy, audit, and push job surfaces.
2. Assistant routing is planner-driven in enclave with schema-constrained output and deterministic fallback (`#180`).
3. Content-blind message flow is active: host remains envelope-only and plaintext is enclave-only (`#146`..`#149`).
4. iOS assistant query UX is integrated and currently focused on polish, response rendering, and regression hardening.

## Immediate Next Engineering Targets

1. Keep expanding deterministic eval/test coverage for planner routing and follow-up continuity.
2. Continue assistant UX hardening for long-form/general-chat rendering in iOS.
3. Preserve and enforce privacy boundary invariants in every backend change.
4. Execute next highest-priority unblocked `phase-1` issue.

## Agent Guardrails

1. Do not remove or bypass privacy controls for convenience.
2. Do not add wide OAuth scopes unless required for a documented feature.
3. Keep edits scoped and consistent with v1 goals.
4. Preserve compatibility between OpenAPI, backend models, and iOS client models.
5. Prefer finishing one issue cleanly over partial progress across many issues.
