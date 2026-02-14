# Observability Alert Drills (Phase I)

This document defines repeatable test scenarios to validate that alerts fire and route correctly.

## Scope

Use this with staging-only endpoints and integrations:
1. PagerDuty service: `alfred-primary`
2. Slack channel: `#alfred-incidents`
3. Dashboards: see `docs/observability-stack-phase1.md`

## Drill Matrix

| Scenario | Trigger | Expected alert | Expected route |
|---|---|---|---|
| API availability burn | Force sustained API 5xx responses above burn threshold for 10 minutes | `alfred-api-availability-burn` | PagerDuty + Slack |
| Job lag high | Pause worker processing to push `max_lag_seconds > 300` for 10 minutes | `alfred-job-lag-high` | PagerDuty + Slack |
| Job failure spike | Inject deterministic permanent failures over baseline for 10 minutes | `alfred-job-failure-spike` | PagerDuty + Slack |
| Push delivery degraded | Force APNs failure mode to push success ratio below 98.5% for 15 minutes | `alfred-push-delivery-degraded` | Slack |

## Execution Procedure

1. Record drill operator, time window, and environment (`staging` only).
2. Apply one scenario trigger from the matrix.
3. Validate metric movement on the matching dashboard panel.
4. Confirm alert opens with expected severity and metadata.
5. Confirm route delivery:
   - PagerDuty `alfred-primary` (where required)
   - Slack `#alfred-incidents`
6. Capture a representative `request_id` and correlate across API request logs, worker metrics/events, and push/audit records.
7. Revert trigger and verify alert recovery/closure.

## Evidence Log Template

| Date | Scenario | Operator | Alert fired | Routing verified | Recovery verified | Notes |
|---|---|---|---|---|---|---|
| 2026-02-14 | API availability burn | TBD | PENDING | PENDING | PENDING | Add links to screenshots/tickets |
| 2026-02-14 | Job lag high | TBD | PENDING | PENDING | PENDING | Add links to screenshots/tickets |
| 2026-02-14 | Job failure spike | TBD | PENDING | PENDING | PENDING | Add links to screenshots/tickets |
| 2026-02-14 | Push delivery degraded | TBD | PENDING | PENDING | PENDING | Add links to screenshots/tickets |

Until all rows are `PASS`, keep alerting work in `IN_PROGRESS`.
