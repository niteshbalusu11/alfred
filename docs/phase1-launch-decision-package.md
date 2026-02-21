# Phase I Launch Decision Package

- Status: Finalized
- Last Updated: 2026-02-21
- Owner: FOUNDER
- Source Issue: #42

## 1) Scope Freeze

### In Scope (Phase I Private Beta)

1. Clerk-based iOS authentication and protected backend API access.
2. Google connector lifecycle (connect, callback, revoke).
3. Privacy controls (revoke, delete-all workflow, audit visibility).
4. LLM-backed assistant queries over connected Google context.
5. Automation v2 runtime: client-defined periodic prompt jobs with enclave-only plaintext and encrypted push rendering.
6. APNs delivery path with Notification Service Extension decrypt/render for encrypted automation notifications.

### Out of Scope (Phase I)

1. Smart-home control and direct actuator integrations.
2. Android/web clients.
3. Broad autonomous high-risk external actions.
4. Legacy hardcoded proactive worker flows as a supported long-term path.

## 2) Beta KPI Targets

| KPI | Definition | Target | Owner | Measurement Source | Review Cadence |
|---|---|---|---|---|---|
| Activation | New beta users who complete Clerk sign-in and Google connect within 24h of invite acceptance | >= 70% weekly | FOUNDER | Clerk auth events + connectors state (`/v1/connectors`) + iOS funnel telemetry | Weekly |
| Reminder Success | Scheduled reminder/automation notifications delivered within 60s of due time | >= 99.5% rolling 7d | BE | Worker metrics + push delivery metrics + audit event timestamps | Daily + weekly rollup |
| D7 Retention | Activated users with at least one assistant interaction or automation-triggered notification on day 7 (+/- 1 day) | >= 35% cohort | FOUNDER | Assistant query events + automation run/push events (redacted metadata) | Weekly |

## 3) Urgent-Email Criteria (v1)

Urgent-email notifications are rules-first and safety-gated.

1. LLM output must satisfy typed schema validation.
2. Actionable notify path is accepted only when:
   1. `should_notify=true`
   2. urgency is `high` or `critical`
   3. rationale and recommended actions are present
3. If any safety/policy check fails, deterministic fallback is used and notification is suppressed.
4. Quiet-hours suppression applies before notification delivery.
5. Host-side logs and audit records remain metadata-only; no plaintext email body logging.

Current behavior alignment references:

1. `docs/llm-safety-layer.md`
2. `docs/content-blindness-invariants.md`
3. `docs/product-context.md`

## 4) Go/No-Go Checklist (Measurable Gates)

| Gate | Required Pass Condition | Evidence Source | Owner |
|---|---|---|---|
| Security Findings | Zero unresolved critical findings; any deferred high finding has approved risk record | `docs/security-assessment-program-phase1.md`, linked GitHub issues | SEC |
| Privacy Boundary | Enclave-only plaintext invariants verified in automated checks; no known plaintext host persistence regressions | `docs/content-blindness-invariants.md`, boundary/integration tests | BE + SEC |
| Backend Quality | `just backend-verify` and `just backend-tests` passing on merge queue | CI `Backend Checks` + local verification logs | BE |
| iOS Quality | `just ios-build` and core tests green for release branch | CI `iOS Build` + `just ios-test` where core logic changed | IOS |
| Reliability | Worker lease/retry/idempotency path healthy; lag/failure alerts configured and tested | `docs/observability-stack-phase1.md`, `docs/observability-alert-drills.md` | SRE |
| APNs Path | Device registration + encrypted payload + NSE decrypt path validated end-to-end in beta env | Issues `#51`, `#213`, `#214`, iOS validation run notes | IOS + BE |
| Delete-All SLA | Delete-all completion monitored and within 24h SLA target | `docs/privacy-delete-sla-monitoring.md` | SEC |
| Launch Readiness | All open P0 blockers resolved or explicitly deferred with approved risk decision | `docs/phase1-master-todo.md` + issue tracker | FOUNDER |

Go/no-go rule:

1. Any failed gate above is a no-go until remediated or explicitly risk-accepted by FOUNDER + SEC.

## 5) Beta Support Severity and SLA Matrix

| Severity | Definition | Initial Response SLA | Mitigation/Containment Target | Escalation |
|---|---|---|---|---|
| Sev-1 Critical | Data exposure risk, auth bypass, widespread outage, or push/automation failure impacting most active users | <= 15 minutes | <= 4 hours | Page SEC + BE + SRE + FOUNDER immediately |
| Sev-2 High | Major feature degradation with viable workaround for subset of users | <= 1 hour | <= 1 business day | Escalate to BE/IOS owner and FOUNDER |
| Sev-3 Medium | Localized bug or reliability issue with low blast radius | <= 1 business day | <= 5 business days | Route to owning team backlog with due date |
| Sev-4 Low | Cosmetic/docs/non-blocking issue | <= 3 business days | Next planned iteration | Standard triage |

Operational notes:

1. Sev-1/Sev-2 incidents require timeline notes and post-incident action items.
2. Support SLAs are measured in Pacific Time business calendar except Sev-1, which is continuous.

## 6) Approval and Change Control

1. This package finalizes board items: `PROD-001`, `PROD-002`, `PROD-003`, `PROD-004`, `PROD-006`, `PROD-007`.
2. Future changes to scope/KPIs/gates/SLAs require:
   1. explicit PR updating this document
   2. linked issue with rationale
   3. updated `docs/phase1-master-todo.md` status/alignment notes
