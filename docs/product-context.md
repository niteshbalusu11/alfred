# Alfred Product Context (Canonical)

- Last Updated: 2026-02-20
- Audience: Engineers, coding agents, product collaborators
- Purpose: Provide shared context on what Alfred is, why it exists, and how to build it safely.

## 1) What We Are Building

Alfred is a hosted, privacy-first AI life assistant.

Phase I (v1) focuses on iOS and Google integrations with LLM-backed assistant behavior:

1. Meeting reminders from Google Calendar
2. Daily morning brief
3. Urgent email alerts from Gmail
4. Natural-language assistant queries over connected Google context
5. Active migration (`#208`): replace hardcoded proactive worker jobs with client-defined periodic automation jobs, while keeping plaintext enclave-only

Alfred is not trying to be a generic chatbot. It is a proactive assistant that takes useful actions and sends timely nudges with high reliability and explicit privacy controls.

## 2) Why This Product Exists

People manage life through fragmented apps (email, calendar, reminders, household logistics). Existing assistants are either:

1. Too siloed (single ecosystem only), or
2. Too technical (self-hosted setups), or
3. Not trustworthy enough on privacy and control.

Alfred’s thesis:

1. Hosted convenience can coexist with strong privacy engineering.
2. Users will trust proactive automation only if control and transparency are first-class.

## 3) Product Goals

1. Save user attention every day through proactive reminders and summaries.
2. Build user trust through strict data minimization, revocation, and deletion controls.
3. Deliver reliable automations before expanding scope.

## 4) Non-Goals for Phase I

1. Smart-home control (Ring/Hue/Nest/HomeKit actions)
2. Android/web clients
3. Fully autonomous high-risk external actions
4. Broad “do everything” assistant behavior

If work does not advance Phase I goals, it should not be prioritized.

## 5) Target User (Phase I)

Primary:

1. Busy professionals who live in Gmail + Calendar and want proactive, low-friction assistance.

They care most about:

1. Reliability
2. Trust/privacy
3. Time savings

## 6) Product Principles

1. Privacy before convenience when tradeoffs appear.
2. Reliability over feature breadth.
3. Minimal permissions by default.
4. Explainability and user control (activity logs, revoke, delete-all).
5. Incremental scope expansion only after stability.

## 7) Architecture Direction (High Level)

Core components:

1. iOS app (`SwiftUI`) for onboarding, settings, and notifications.
2. Rust API server (`axum`) for Clerk-authenticated connector/preferences/privacy APIs.
3. LLM orchestration layer for assistant query + proactive summary generation.
4. OpenRouter provider gateway with backend-controlled model routing/fallback.
5. Rust worker (`tokio`) for scheduled and proactive jobs.
6. Encrypted Postgres for operational state.
7. TEE-backed sensitive execution path for token/data decryption and provider fetches.
8. APNs pipeline for user notifications.
9. Notification Service Extension decrypt/render path on iOS for encrypted automation push payloads.

Reference docs:

1. RFC: `/Users/niteshchowdharybalusu/Documents/alfred/docs/rfc-0001-alfred-ios-v1.md`
2. OpenAPI: `/Users/niteshchowdharybalusu/Documents/alfred/api/openapi.yaml`
3. Phase I execution board: `/Users/niteshchowdharybalusu/Documents/alfred/docs/phase1-master-todo.md`
4. Phase I launch decision package: `/Users/niteshchowdharybalusu/Documents/alfred/docs/phase1-launch-decision-package.md`

## 8) Privacy and Security Strategy

This is a hosted product, so privacy must be engineered into architecture, not assumed.

Required controls:

1. Strict OAuth scope minimization.
2. Managed identity provider controls for sign-in/session security (Clerk direction for Phase I migration).
3. Encrypted secret/token storage.
4. Sensitive decryption/processing in attested trusted environments.
5. Redacted logs and auditability.
6. User controls for revoke and delete-all.
7. No silent broadening of data access.
8. LLM prompt-injection safeguards and output schema validation before user-visible actions.
9. Redacted LLM telemetry (model/latency/usage) without raw sensitive payload logging.
10. Assistant intent resolution must remain enclave semantic-planner driven (no keyword-only routing in primary path).
11. Automation prompt/output plaintext must remain enclave-only; host handles metadata + encrypted payloads only.

Operating rule:

1. If a feature weakens trust boundaries, redesign it or defer it.

## 9) Definition of Success (Phase I)

Functional success:

1. Users can connect Google reliably.
2. Reminder/brief/urgent-email flows work end-to-end with LLM-backed summarization/prioritization.
3. Revoke and delete-all are real and verifiable.
4. Users can ask assistant questions (for example, meetings today) and receive accurate, concise answers.

Quality success:

1. Backend completion gate passes (`fmt + clippy + tests + build`).
2. iOS app builds cleanly; logic tests added when core logic changes.

Operational success:

1. Baseline observability is in place (latency, failures, job lag, push outcomes).
2. Critical incidents have runbooks.

## 10) Phase I Execution Model

Execution queue:

1. GitHub issues labeled `phase-1`.
2. Priority order: `P0` first, then `P1`.
3. LLM backend migration issues use label `ai-backend`.

Planning control board:

1. `/Users/niteshchowdharybalusu/Documents/alfred/docs/phase1-master-todo.md`

Sync rule:

1. GitHub issue state and board status should remain aligned.

## 11) Roadmap Shape (After Phase I)

Only after Phase I quality and trust benchmarks are met:

1. Expand channels/platforms (Android/web)
2. Add additional integrations
3. Add deeper proactive automation
4. Consider broader “life OS” surfaces

## 12) Guidance for Agents

When making decisions, optimize for this order:

1. User trust and privacy
2. Correctness/reliability
3. Scope discipline (finish current issue well)
4. Implementation speed

If uncertain, choose the safer and narrower implementation and document tradeoffs in the issue.
