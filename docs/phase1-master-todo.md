# Phase I Master Todo (Execution Board)

- Project: Alfred (iOS + Hosted Backend + TEE-sensitive processing)
- Created: 2026-02-14
- Updated: 2026-02-21 (issues `#41` and `#42`)
- Scope: Phase I private beta readiness

## 1) Phase I Outcome

Ship a private beta where iOS users can:

1. Connect Google account securely.
2. Receive meeting reminders.
3. Receive daily morning brief.
4. Receive urgent email alerts.
5. View activity logs and revoke/delete data.
6. Ask natural-language assistant questions about connected Google context (LLM-backed).

## 2) Status + Priority Legend

- Priority: `P0` = critical path, `P1` = important but not launch-blocking.
- Status: `TODO`, `IN_PROGRESS`, `BLOCKED`, `DONE`.

## 3) Owner Abbreviations

- `FOUNDER`: Product/PM decisions
- `IOS`: iOS engineer
- `BE`: Backend engineer
- `SEC`: Security engineer
- `SRE`: Infra/DevOps
- `QA`: QA/testing owner

## 4) Milestones (Target Dates)

1. `M0` Foundation complete by **February 27, 2026**
2. `M1` Secure data path + OAuth complete by **March 13, 2026**
3. `M2` End-to-end feature complete by **April 3, 2026**
4. `M3` Beta readiness + launch gates complete by **April 24, 2026**

---

## Auth Direction Update (2026-02-14)

1. Phase I auth direction is now Clerk-based (GitHub epic: `#52`).
2. New auth work should target Clerk migration issues (`#53`, `#54`, `#56`).
3. Breaking auth changes are acceptable at this phase to move faster.
4. Custom `/v1/auth/ios/session*` implementation should be removed or hard-disabled after Clerk integration.

## AI Backend Direction Update (2026-02-15)

1. Phase I backend direction is now LLM-first for assistant summarization and question answering.
2. OpenRouter is the default provider gateway with model routing/fallback handled in backend.
3. Rule-based urgent-email logic has been removed from production worker paths (`#91`).
4. Execution queue for this migration is GitHub issues `#91` through `#103` (`ai-backend` label).

## Assistant Semantic Planner Update (2026-02-18)

1. Keyword-driven assistant routing replacement is complete via tracker issue `#180`.
2. Planner routing is now schema-constrained and enclave-resident with deterministic fallback/clarification.
3. Phase sequence completed:
   1. Phase A: planner contract and normalization plumbing
   2. Phase B: executor integration and clarification path
   3. Phase C: English-first temporal robustness policy
   4. Phase D: telemetry, eval hardening, and test expansion
4. Product scope decision for this line: English-first only for now; multilingual expansion deferred.

## Content Blindness Privacy Boundary Update (2026-02-17)

1. Current state: connector provider fetch/decrypt paths and assistant message-body flows are enclave-plaintext-only with ciphertext host transit/storage.
2. Privacy goal: server can observe routing/operational metadata, while message content remains ciphertext outside enclave memory.
3. Migration source of truth is tracker issue `#146` with phases `#147`, `#148`, and `#149`.
4. Labels for this line of work: `phase-1`, `P0`, `backend`, `content-blindness`.
5. Breaking protocol/contract changes are explicitly acceptable pre-launch for this migration.
6. `#149` hardening completed: OAuth code exchange moved to enclave path, job payload storage is encrypted-at-write, Redis reliability state is metadata-only, and privacy guard tests/docs were expanded.

## Automation v2 Direction Update (2026-02-21)

1. Proactive backend architecture is now automation-first via client-defined periodic prompt jobs (tracker `#208`).
2. Completed delivery phases:
   1. Schema/repositories (`#209`)
   2. API CRUD (`#210`)
   3. Generic worker scheduler/executor (`#211`)
   4. Enclave automation execution + encrypted artifacts (`#212`)
   5. Encrypted APNs payload contract (`#213`)
   6. iOS Notification Service Extension decrypt/render (`#214`)
   7. iOS Automations tab + scheduler UI (`#219`)
3. Legacy hardcoded proactive flows are treated as replaced by the automation runtime line.

## 5) Execution Board

### A) Product and Scope Control

| ID | Pri | Task | Owner | ETA | Status | Depends On | Exit Criteria |
|---|---|---|---|---|---|---|---|
| PROD-001 | P0 | Freeze Phase I scope doc | FOUNDER | 2026-02-16 | DONE | - | Scope finalized in `docs/phase1-launch-decision-package.md` |
| PROD-002 | P0 | Freeze out-of-scope list (no smart-home control) | FOUNDER | 2026-02-16 | DONE | PROD-001 | Out-of-scope list finalized in `docs/phase1-launch-decision-package.md` |
| PROD-003 | P0 | Define beta KPIs (activation, D7 retention, reminder success) | FOUNDER | 2026-02-18 | DONE | PROD-001 | KPI targets documented in `docs/phase1-launch-decision-package.md` |
| PROD-004 | P0 | Finalize urgent-email criteria (LLM-first with deterministic fallback) | FOUNDER | 2026-02-18 | DONE | PROD-001 | Urgent-email rules/policy documented in `docs/phase1-launch-decision-package.md` |
| PROD-005 | P1 | Finalize push copy/content policy | FOUNDER | 2026-02-19 | TODO | PROD-004 | Copy approved |
| PROD-006 | P0 | Define launch/no-launch checklist | FOUNDER | 2026-02-20 | DONE | PROD-003 | Checklist version 1 finalized in `docs/phase1-launch-decision-package.md` |
| PROD-007 | P1 | Define beta support SLAs and severity matrix | FOUNDER | 2026-02-21 | DONE | PROD-006 | Severity/SLA matrix published in `docs/phase1-launch-decision-package.md` |

### B) API and Backend Core

| ID | Pri | Task | Owner | ETA | Status | Depends On | Exit Criteria |
|---|---|---|---|---|---|---|---|
| BE-001 | P0 | Migrate backend auth to Clerk JWT verification and identity mapping | BE | 2026-02-26 | DONE | PROD-001 | Protected endpoints authorize Clerk tokens and map Clerk subject to stable user identity |
| BE-002 | P0 | Add health/readiness endpoints | BE | 2026-02-20 | DONE | - | `/healthz` and `/readyz` live |
| BE-003 | P0 | Add structured logging with request_id | BE | 2026-02-21 | DONE | BE-002 | Request logs include trace fields |
| BE-004 | P0 | Standardize API error envelope/codes | BE | 2026-02-22 | TODO | BE-001 | All endpoints use common error format |
| BE-005 | P0 | Implement `/v1/connectors/google/start` real OAuth URL + state | BE | 2026-03-01 | DONE | BE-004 | Endpoint returns valid provider URL/state |
| BE-006 | P0 | Implement `/v1/connectors/google/callback` token exchange | BE | 2026-03-03 | DONE | BE-005 | Real token exchange succeeds |
| BE-007 | P0 | Implement `/v1/connectors/{id}` revoke + provider-side revoke | BE | 2026-03-05 | DONE | BE-006 | Connector revoke fully works |
| BE-008 | P0 | Implement `/v1/preferences` persistence | BE | 2026-03-06 | DONE | DB-001 | Read/write preferences backed by DB |
| BE-009 | P0 | Implement `/v1/audit-events` pagination and filters | BE | 2026-03-10 | TODO | DB-004 | Cursor pagination works |
| BE-010 | P0 | Implement `/v1/privacy/delete-all` async job trigger | BE | 2026-03-12 | DONE | DB-007 | Delete request queued and trackable |
| BE-011 | P1 | Add endpoint-level rate limiting | BE | 2026-03-14 | DONE | BE-004 | Rate-limits enforced |
| BE-012 | P1 | OpenAPI drift check in CI | BE | 2026-03-14 | TODO | BE-004 | CI fails on contract drift |
| BE-013 | P1 | Refactor oversized security-critical backend modules for maintainability | BE | 2026-03-16 | DONE | BE-006, WRK-007 | `worker/src/main.rs` and `http/connectors.rs` decomposed into focused modules with behavior parity |
| BE-014 | P0 | Deprecate legacy custom auth endpoints and align contracts/docs to Clerk | BE | 2026-03-04 | DONE | BE-001, IOS-001 | Legacy `/v1/auth/ios/session*` endpoints removed or disabled-by-default and docs/contracts updated |

### C) Database and Migrations

| ID | Pri | Task | Owner | ETA | Status | Depends On | Exit Criteria |
|---|---|---|---|---|---|---|---|
| DB-001 | P0 | Wire Postgres in backend (`sqlx`) | BE | 2026-02-24 | DONE | - | App connects and queries DB |
| DB-002 | P0 | Convert draft SQL into migration sequence | BE | 2026-02-24 | DONE | DB-001 | `migrate up` produces schema |
| DB-003 | P0 | Add token encryption metadata fields (version/rotated_at) | BE | 2026-02-27 | DONE | DB-002 | Schema supports key rotation |
| DB-004 | P0 | Add audit_events indexes and query plan checks | BE | 2026-03-01 | TODO | DB-002 | Audit endpoint query < target latency |
| DB-005 | P0 | Add jobs table locking/lease fields | BE | 2026-03-02 | DONE | DB-002 | Worker-safe leasing possible |
| DB-006 | P1 | Add dead-letter table for failed jobs | BE | 2026-03-04 | DONE | DB-005 | Failed job archival works |
| DB-007 | P0 | Add privacy_delete_requests workflow states | BE | 2026-03-05 | DONE | DB-002 | Delete-all flow persists state |
| DB-008 | P1 | Add retention policy job schema support | BE | 2026-03-10 | TODO | DB-004 | Retention window stored/configured |
| DB-009 | P1 | Migration smoke tests in CI | QA | 2026-03-12 | TODO | DB-002 | CI executes migrations cleanly |

### D) TEE and Secrets (Critical Path)

| ID | Pri | Task | Owner | ETA | Status | Depends On | Exit Criteria |
|---|---|---|---|---|---|---|---|
| SEC-001 | P0 | Pick TEE provider/architecture decision record | SEC | 2026-02-21 | DONE | - | ADR approved |
| SEC-002 | P0 | Build enclave image baseline | SEC | 2026-02-28 | DONE | SEC-001 | Image boot + smoke pass |
| SEC-003 | P0 | Implement enclave attestation validation | SEC | 2026-03-04 | DONE | SEC-002 | Attestation verified end-to-end |
| SEC-004 | P0 | Bind KMS decrypt access to enclave measurements | SEC | 2026-03-06 | DONE | SEC-003 | Decrypt denied outside attested enclave |
| SEC-005 | P0 | Implement secure host<->enclave RPC contract | SEC | 2026-03-08 | DONE | SEC-002 | Versioned HMAC-authenticated RPC contract live with replay protection and tests |
| SEC-006 | P0 | Move Google API fetch/decrypt path into enclave process | SEC | 2026-03-13 | DONE | SEC-004, BE-006 | Sensitive path enclave-only (issue #126) |
| SEC-007 | P0 | Token encryption/decryption service with key versioning | SEC | 2026-03-09 | DONE | SEC-004 | Key versioned crypto works with idempotent rotation/repair workflow |
| SEC-008 | P1 | Add key rotation runbook + scripts | SEC | 2026-03-15 | DONE | SEC-007 | Rotation runbook + operator script with dry-run/env targeting (`docs/tee-kms-rotation-runbook.md`, `scripts/security/tee-kms-rotation.sh`) |
| SEC-009 | P0 | Secrets never logged tests and lint checks | SEC | 2026-03-10 | DONE | BE-003 | No secret leakage in logs (guard coverage + metadata value redaction hardening) |
| SEC-010 | P0 | Threat model review (STRIDE) | SEC | 2026-03-17 | DONE | SEC-006 | Signed threat model doc |
| SEC-011 | P0 | External security assessment prep | SEC | 2026-04-03 | IN_PROGRESS | SEC-010 | Scope + test plan baseline documented (`docs/security-assessment-program-phase1.md`) |
| SEC-012 | P0 | Remediate critical findings before beta | SEC | 2026-04-17 | TODO | SEC-011 | No open critical findings |

### E) Worker, Scheduling, and Proactive Jobs

| ID | Pri | Task | Owner | ETA | Status | Depends On | Exit Criteria |
|---|---|---|---|---|---|---|---|
| WRK-001 | P0 | Implement due-job fetch with row leasing | BE | 2026-03-06 | DONE | DB-005 | No duplicate processing on concurrency test |
| WRK-002 | P0 | Implement retry policy (transient/permanent) | BE | 2026-03-08 | DONE | WRK-001 | Retries respect policy |
| WRK-003 | P1 | Implement dead-letter writes | BE | 2026-03-09 | DONE | DB-006, WRK-002 | Failed jobs land in DLQ table |
| WRK-004 | P0 | Implement meeting reminder job | BE | 2026-03-12 | DONE | WRK-001, APNS-001 | Reminder push end-to-end works |
| WRK-005 | P0 | Implement morning brief job (legacy baseline) | BE | 2026-03-15 | DONE | WRK-001, APNS-001 | Legacy brief baseline delivered and retired by AI-000 cleanup |
| WRK-006 | P0 | Implement urgent-email scan job (legacy rule baseline) | BE | 2026-03-18 | DONE | BE-006, WRK-001 | Legacy urgent baseline delivered and retired by AI-000 cleanup |
| WRK-007 | P0 | Add idempotency keys for outbound actions | BE | 2026-03-14 | DONE | WRK-002 | Duplicate sends prevented |
| WRK-008 | P1 | Add per-user concurrency limits | BE | 2026-03-19 | DONE | WRK-001 | Limits enforced in worker |
| WRK-009 | P1 | Add worker lag metrics and alerts | SRE | 2026-03-20 | TODO | OBS-001 | Lag dashboard + alert live |
| WRK-010 | P1 | Add outage recovery/backfill command | BE | 2026-03-22 | TODO | WRK-001 | Backfill tested in staging |

### F) APNs and Notification Pipeline

| ID | Pri | Task | Owner | ETA | Status | Depends On | Exit Criteria |
|---|---|---|---|---|---|---|---|
| APNS-001 | P0 | Configure APNs credentials per environment | IOS | 2026-03-01 | TODO | - | Dev/staging push can be sent |
| APNS-002 | P0 | Implement device registration endpoint persistence | BE | 2026-03-02 | DONE | DB-002 | Device token saved/updated |
| APNS-003 | P0 | Wire iOS token registration call on app start | IOS | 2026-03-05 | TODO | APNS-002 | Device appears in backend DB |
| APNS-004 | P0 | Implement push send service in backend | BE | 2026-03-09 | DONE | APNS-001 | Backend sends push successfully |
| APNS-005 | P1 | Add retry handling for APNs transient failures | BE | 2026-03-11 | DONE | APNS-004 | Retry behavior validated |
| APNS-006 | P1 | Add quiet-hours suppression in send path | BE | 2026-03-13 | DONE | BE-008, APNS-004 | Quiet hour rule enforced |
| APNS-007 | P1 | Add redacted notification audit records | BE | 2026-03-13 | DONE | DB-004, APNS-004 | Notification logs visible |

### G) iOS App Delivery

| ID | Pri | Task | Owner | ETA | Status | Depends On | Exit Criteria |
|---|---|---|---|---|---|---|---|
| IOS-001 | P0 | Integrate Clerk iOS auth and API token provider wiring | IOS | 2026-02-27 | DONE | BE-001 | App obtains Clerk token and authenticated API calls succeed |
| IOS-002 | P0 | Native bottom-tab app shell with per-tab navigation stacks (FE02) | IOS | 2026-02-16 | DONE | IOS-013 | Duplicate of IOS-014 retained for historical traceability |
| IOS-003 | P0 | Build Google connect UI flow + Connectors hub v1 (FE05) | IOS | 2026-03-06 | DONE | BE-005 | Connectors tab shows Google state/actions, error+retry UX, and extensible future-provider layout |
| IOS-013 | P0 | Dark-mode theme tokens + shared UI primitives (FE01) | IOS | 2026-02-16 | DONE | - | App uses dark-only tokens and shared components |
| IOS-014 | P0 | Build native tabbed app shell (FE02) | IOS | 2026-02-15 | DONE | IOS-013 | TabView + per-tab NavigationStack with centralized tab routing |
| IOS-015 | P0 | Home screen v1 redesign (FE03) | IOS | 2026-02-15 | DONE | IOS-014 | Home screen shows summary, status cards, quick actions, and loading/empty/error states |
| IOS-004 | P0 | Build preferences screen | IOS | 2026-03-08 | DONE | BE-008 | Preferences read/write works |
| IOS-005 | P0 | Build activity log screen | IOS | 2026-03-12 | DONE | BE-009 | Activity screen v1 delivered (`#70`); top-tab entry later replaced by Automations (`#219`) |
| IOS-006 | P0 | Build privacy controls (revoke + delete-all) | IOS | 2026-03-14 | DONE | BE-007, BE-010 | Revoke/delete flows complete |
| IOS-007 | P1 | Add offline/error state UI patterns | IOS | 2026-03-16 | TODO | IOS-003 | UX handles API failures cleanly |
| IOS-008 | P1 | Add analytics events (privacy-safe) | IOS | 2026-03-18 | TODO | PROD-003 | KPI events emitting |
| IOS-009 | P1 | Add feature flags for staged rollout | IOS | 2026-03-20 | TODO | IOS-002 | Features can be toggled remotely |
| IOS-010 | P1 | Wire local package client to production API base config | IOS | 2026-03-21 | TODO | IOS-001 | Environment switching works |
| IOS-011 | P1 | Improve notification deep-link handling | IOS | 2026-03-22 | TODO | APNS-003 | Tap opens correct app screen |
| IOS-012 | P1 | App Store/TestFlight beta metadata prep | IOS | 2026-04-10 | TODO | IOS-006 | Build ready for tester distribution |

### H) Observability and Reliability

| ID | Pri | Task | Owner | ETA | Status | Depends On | Exit Criteria |
|---|---|---|---|---|---|---|---|
| OBS-001 | P0 | Define core service metrics (API, jobs, push) | SRE | 2026-03-01 | DONE | BE-003 | Metrics spec approved |
| OBS-002 | P0 | Add metrics instrumentation to API | BE | 2026-03-08 | DONE | OBS-001 | API dashboards populated |
| OBS-003 | P0 | Add metrics instrumentation to worker | BE | 2026-03-10 | DONE | OBS-001 | Worker dashboards populated |
| OBS-004 | P0 | Add alerting for job lag and failure spikes | SRE | 2026-03-15 | IN_PROGRESS | OBS-002, OBS-003 | Alerts firing in test drills |
| OBS-005 | P1 | Add tracing across API->worker->push path | SRE | 2026-03-18 | DONE | OBS-002 | Trace spans visible |
| OBS-006 | P0 | SLO definition and dashboard | SRE | 2026-03-20 | DONE | OBS-004 | SLO page + dashboard contracts published (`docs/observability-stack-phase1.md`) |
| OBS-007 | P1 | Backup + restore rehearsal for Postgres | SRE | 2026-03-25 | TODO | DB-002 | Restore drill report approved |
| OBS-008 | P1 | Runbook for top 5 incidents | SRE | 2026-03-28 | IN_PROGRESS | OBS-004 | Runbooks available in docs |

### I) Testing and QA

| ID | Pri | Task | Owner | ETA | Status | Depends On | Exit Criteria |
|---|---|---|---|---|---|---|---|
| QA-001 | P0 | Unit tests for shared models and validation | QA | 2026-03-05 | TODO | BE-004 | Core model tests passing |
| QA-002 | P0 | API integration tests (auth, connectors, prefs, audit) | QA | 2026-03-12 | TODO | BE-010 | CI integration suite green |
| QA-003 | P0 | Worker tests for retries/idempotency | QA | 2026-03-16 | TODO | WRK-007 | Duplicate prevention verified |
| QA-004 | P0 | iOS onboarding/connect flow UI tests | QA | 2026-03-18 | TODO | IOS-003 | UI smoke tests passing |
| QA-005 | P0 | End-to-end test: connect->schedule->notification | QA | 2026-03-22 | TODO | WRK-004, APNS-004 | E2E scenario repeatable |
| QA-006 | P1 | Load test due-job spike handling | QA | 2026-03-26 | TODO | WRK-001 | Service meets throughput target |
| QA-007 | P1 | Chaos test API/worker restarts | QA | 2026-03-30 | TODO | OBS-004 | Recovery objectives met |
| QA-008 | P0 | Pre-beta regression suite | QA | 2026-04-15 | TODO | QA-001..007 | Regression suite passes |

### J) Privacy, Compliance, and Launch Readiness

| ID | Pri | Task | Owner | ETA | Status | Depends On | Exit Criteria |
|---|---|---|---|---|---|---|---|
| GOV-001 | P0 | Data retention matrix by table/event type | SEC | 2026-03-15 | TODO | DB-008 | Retention policy approved |
| GOV-002 | P0 | Privacy policy draft aligned with real architecture | FOUNDER | 2026-03-20 | TODO | SEC-010 | Legal-review ready draft |
| GOV-003 | P0 | Terms + connector consent language | FOUNDER | 2026-03-22 | TODO | PROD-005 | Approved UX/legal copy |
| GOV-004 | P0 | Delete-all SLA monitoring | SEC | 2026-03-25 | DONE | BE-010 | Delete requests tracked vs SLA |
| GOV-005 | P1 | Data export workflow (basic) | BE | 2026-03-28 | TODO | DB-002 | Export available for support |
| GOV-006 | P0 | Incident response plan + notification template | SEC | 2026-04-01 | TODO | OBS-008 | Playbook approved |
| GOV-007 | P0 | External pentest and remediation complete | SEC | 2026-04-17 | TODO | SEC-011 | No critical unresolved issues |
| GOV-008 | P0 | Final go/no-go review meeting | FOUNDER | 2026-04-23 | TODO | All P0 done | Decision logged |
| GOV-009 | P0 | Private beta launch | FOUNDER | 2026-04-24 | TODO | GOV-008 | First cohort onboarded |

### K) AI Assistant and LLM Backend

| ID | Pri | Task | Owner | ETA | Status | Depends On | Exit Criteria |
|---|---|---|---|---|---|---|---|
| AI-000 | P0 | Remove rule-based assistant logic and legacy backend paths (`#91`) | BE | 2026-03-29 | DONE | - | Rule-based assistant decision path removed from production |
| AI-001 | P0 | Add LLM gateway abstraction + typed output contracts (`#92`) | BE | 2026-03-06 | DONE | - | Provider-agnostic contract merged with schema validation |
| AI-002 | P0 | Implement OpenRouter adapter + routing/fallback controls (`#93`) | BE | 2026-03-08 | DONE | AI-001 | Backend can execute LLM requests via OpenRouter with retries |
| AI-003 | P0 | Build Google context assembler for LLM prompts (`#94`) | BE | 2026-03-10 | DONE | AI-001, AI-002 | Deterministic context payloads generated for assistant capabilities |
| AI-004 | P0 | Add `/v1/assistant/query` endpoint for interactive questions (`#95`) | BE | 2026-03-12 | DONE | AI-002, AI-003 | Meetings query works end-to-end with typed assistant response |
| AI-005 | P0 | Add LLM safety layer + deterministic fallback (`#96`) | SEC | 2026-03-14 | DONE | AI-001, AI-002 | Injection defenses and schema/policy guards enforced |
| AI-006 | P0 | Migrate morning brief worker path to LLM orchestration (`#97`) | BE | 2026-03-16 | DONE | AI-003, AI-005 | Morning brief push content is LLM-generated and policy-safe |
| AI-007 | P0 | Migrate urgent-email worker path to LLM prioritization (`#98`) | BE | 2026-03-18 | DONE | AI-003, AI-005 | Urgent-email decision path is LLM-based with safe fallback |
| AI-008 | P0 | Add AI observability + redacted audit events (`#99`) | SRE | 2026-03-20 | DONE | AI-004, AI-006, AI-007 | Latency/usage/cost metrics and redacted audit events are live |
| AI-009 | P0 | Add LLM reliability guardrails (`#100`) | BE | 2026-03-22 | DONE | AI-004 | Circuit breaker/rate limits/cache/budgets enforced |
| AI-013 | P0 | Move LLM reliability state to Redis (`#118`) | BE | 2026-03-30 | DONE | AI-009 | Reliability cache/rate-limit/breaker/budget state is shared and restart-safe |
| AI-010 | P0 | Add assistant session memory for follow-up continuity (`#101`) | BE | 2026-03-24 | DONE | AI-004 | Session-context follow-up queries supported with retention controls |
| AI-011 | P0 | Add LLM eval/regression harness in CI (`#102`) | QA | 2026-03-26 | DONE | AI-005, AI-006, AI-007 | Prompt/output regressions are detected by automated checks |
| AI-012 | P0 | Maintain migration tracker + execution order (`#103`) | FOUNDER | 2026-03-05 | DONE | - | Tracker issue reflects live execution order and status |
| AI-014 | P0 | Replace keyword assistant routing with enclave semantic planner (`#180`) | BE | 2026-02-18 | DONE | AI-004, AI-005, AI-010 | Planner-driven capability resolution is production path with schema validation, policy gating, and deterministic fallback |
| AI-015 | P0 | Planner rollout phases A-D (contract, integration, English temporal policy, hardening) (`#181`..`#187`) | BE | 2026-02-18 | DONE | AI-014 | All planned rollout phases delivered with unit/integration/eval coverage and deep-review pass |

### L) Content Blindness Migration (Server-Blind Message Content)

| ID | Pri | Task | Owner | ETA | Status | Depends On | Exit Criteria |
|---|---|---|---|---|---|---|---|
| CB-001 | P0 | Define enclave-only message-content architecture and attested key agreement (`#147`) | BE | 2026-02-20 | DONE | SEC-006, AI-004 | Approved protocol/spec defines ciphertext-only server relay and enclave-only plaintext boundary (`docs/assistant-attested-encryption-v1.md`) |
| CB-002 | P0 | Implement encrypted message transport + enclave decryption/processing path (`#148`) | BE | 2026-02-27 | DONE | CB-001 | `/v1/assistant/query`-class flows carry ciphertext through control plane; plaintext exists only in enclave runtime |
| CB-003 | P0 | Add privacy verification gates, redaction tests, and rollout hardening (`#149`) | BE | 2026-03-03 | DONE | CB-002 | Automated checks and audit evidence confirm message-body server blindness with metadata-only server observability (`docs/content-blindness-invariants.md`, boundary guard tests) |

### M) Automation v2 Migration (Client-Defined Periodic Prompt Jobs)

| ID | Pri | Task | Owner | ETA | Status | Depends On | Exit Criteria |
|---|---|---|---|---|---|---|---|
| AUTO-001 | P0 | Add automation schema + repository layer (`#209`) | BE | 2026-02-20 | DONE | DB-002 | `automation_rules` lifecycle is persisted with scheduling metadata |
| AUTO-002 | P0 | Implement automation CRUD API with encrypted prompt envelope (`#210`) | BE | 2026-02-21 | DONE | AUTO-001, SEC-003 | iOS can create/update/pause/delete rules without plaintext host-boundary regressions |
| AUTO-003 | P0 | Replace hardcoded worker actions with generic scheduler/executor (`#211`) | BE | 2026-02-21 | DONE | AUTO-001, WRK-001 | Worker executes due `AUTOMATION_RUN` jobs with lease/retry/idempotency guarantees |
| AUTO-004 | P0 | Add enclave automation execution + encrypted notification artifacts (`#212`) | SEC | 2026-02-21 | DONE | AUTO-003, SEC-005 | Automation prompt/output plaintext remains enclave-only |
| AUTO-005 | P0 | Ship encrypted APNs payload contract for automations (`#213`) | BE | 2026-02-21 | DONE | AUTO-004, APNS-004 | Host sends encrypted push payload with safe fallback envelope |
| AUTO-006 | P0 | Add iOS Notification Service Extension decrypt/render (`#214`) | IOS | 2026-02-21 | DONE | AUTO-005 | User-visible automation push content is decrypted/rendered on-device |
| AUTO-007 | P0 | Replace Activity tab with Automations scheduler UI (`#219`) | IOS | 2026-02-21 | DONE | AUTO-002, AUTO-006 | User can manage periodic automations from iOS tab UI |
| AUTO-008 | P0 | Close tracker and final migration acceptance (`#208`) | FOUNDER | 2026-02-24 | IN_PROGRESS | AUTO-001..AUTO-007 | Tracker confirms legacy-flow removal + final E2E acceptance criteria closure |

---

## 6) Reconciliation Summary (2026-02-21)

1. Corrected stale statuses where implementation is already merged:
   1. `BE-001`, `BE-002`, `BE-003`, `BE-008`, `BE-014`
   2. `DB-003`
   3. `APNS-002`, `APNS-004`, `APNS-005`, `APNS-006`, `APNS-007`
   4. `IOS-001`
   5. `AI-012`
2. Added missing automation migration board section (`AUTO-001`..`AUTO-008`) aligned to issues `#208`..`#214` and `#219`.
3. Normalized duplicate board rows by explicitly marking `IOS-002` as historical duplicate of `IOS-014`.
4. Updated activity UX status note to reflect the activity-to-automations tab migration (`#70` -> `#219`).
5. Finalized launch decision package scope/KPI/urgent-email/checklist/SLA items (`#42`).

### Evidence Map for DONE Rows

1. Product launch decision package: `PROD-001`, `PROD-002`, `PROD-003`, `PROD-004`, `PROD-006`, `PROD-007` -> `#42`, `docs/phase1-launch-decision-package.md`.
2. Clerk/Auth migration: `BE-001`, `BE-014`, `IOS-001` -> `#52`, `#53`, `#54`, `#56`.
3. Backend/API core + logging/rate limits + modularization: `BE-002`, `BE-003`, `BE-005`, `BE-006`, `BE-007`, `BE-008`, `BE-010`, `BE-011`, `BE-013` -> `#1`, `#6`, `#7`, `#9`, `#24`, `#64`.
4. Database and privacy-state primitives: `DB-001`, `DB-002`, `DB-003`, `DB-005`, `DB-006`, `DB-007`, `GOV-004` -> `#2`, `#4`, `#7`, `#19`, `#127`.
5. TEE/security line: `SEC-001`..`SEC-010` -> `#121`..`#130`.
6. Worker core/reliability primitives: `WRK-001`..`WRK-008` -> `#4`, `#18`, `#91`, `#97`, `#98`, `#211`.
7. APNs pipeline hardening: `APNS-002`, `APNS-004`, `APNS-005`, `APNS-006`, `APNS-007` -> `#5`, `#213`, `#214`.
8. iOS FE core/tab/connect/privacy line: `IOS-002`, `IOS-003`, `IOS-004`, `IOS-005`, `IOS-006`, `IOS-013`, `IOS-014`, `IOS-015` -> `#67`, `#68`, `#69`, `#70`, `#71`, `#72`, `#219`.
9. LLM backend migration: `AI-000`..`AI-015` -> `#91`..`#103`, `#118`, `#180`, `#181`, `#182`, `#185`, `#187`.
10. Content blindness migration: `CB-001`, `CB-002`, `CB-003` -> `#147`, `#148`, `#149`.
11. Automation v2 migration: `AUTO-001`..`AUTO-007` -> `#209`, `#210`, `#211`, `#212`, `#213`, `#214`, `#219`.

## 7) Critical Path (Outstanding as of 2026-02-21)

1. Security assessment and remediation closure: `SEC-011`, `SEC-012`, `GOV-007` (see open `#43`).
2. APNs client auto-registration + in-app validation hardening: `APNS-003` (see open `#51`).
3. Automation migration closure and acceptance sign-off: `AUTO-008` (see open `#208`).
4. Pre-beta verification gates: `QA-002`, `QA-005`, `QA-008`, `IOS-012`.
5. Final launch governance: `GOV-008`, `GOV-009`.

## 8) Open Backlog Alignment (GitHub `phase-1`)

1. Open `P0` issues (post-`#42` merge target state): `#43`, `#51`.
2. Open `P1` issues: `#8`, `#10`, `#25`, `#44`, `#45`, `#46`, `#73`, `#74`, `#75`, `#76`.
3. Reconciliation note: automation tracker `#208` is active P0 architecture work but currently unlabeled; board tracks it under `AUTO-008` to preserve execution visibility.

## 9) Weekly Operating Cadence

1. Weekly planning: update status for all `P0` items.
2. Mid-week risk review: check blocked items and reassign ownership.
3. Weekly demo: show one new end-to-end path.
4. Weekly launch readiness score:
   1. `% P0 done`
   2. Open critical bugs
   3. Security blocker count
   4. SLO trend
