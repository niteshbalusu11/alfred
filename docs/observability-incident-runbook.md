# Observability Incident Runbook

Use this runbook for API/worker/push reliability incidents in staging and production.

## Dashboards

- API Overview: [staging-api-overview](https://grafana.staging.alfred.internal/d/alfred-api-overview)
- Worker Overview: [staging-worker-overview](https://grafana.staging.alfred.internal/d/alfred-worker-overview)
- Push Delivery: [staging-push-overview](https://grafana.staging.alfred.internal/d/alfred-push-overview)
- SLO Burn Rates: [staging-slo-burn](https://grafana.staging.alfred.internal/d/alfred-slo-burn)

Dashboards and SLO panel contracts are defined in:
- `docs/observability-stack-phase1.md`

## Triage Steps

1. Open SLO burn dashboard first and identify the failing SLI.
2. Pivot to service dashboard (API/worker/push) for the same time range.
3. Pull representative `request_id` values from API logs.
4. Correlate with worker job/audit events using `request_id` and `job_id`.
5. Confirm whether failure source is API, queue/worker lag, or APNs delivery.

## Service-Specific Checks

### API

- Look for spikes in `status >= 500` and p95 latency.
- Validate DB readiness and rate-limit behavior (`429` spikes vs expected patterns).

### Worker

- Review `worker tick metrics` for:
  - `max_lag_seconds`
  - `retryable_failures`
  - `permanent_failures`
  - `dead_lettered_jobs`
- Confirm queue drain trend (`pending_due_jobs`) is moving downward.

### Push

- Compare `push_attempts` vs `push_delivered`.
- Inspect `push_transient_failures` vs `push_permanent_failures`.
- Verify APNs endpoint and auth token status.

## Escalation

- High-severity alerts page `alfred-primary` immediately.
- Post incident summary to `#alfred-incidents` with:
  - incident window
  - impacted SLI
  - current hypothesis
  - next mitigation step

## Recovery Criteria

1. Alerts clear for at least 15 minutes.
2. SLI recovers to within target thresholds.
3. Incident note includes root cause and follow-up actions.

## Reliability Readiness Checklist

1. Dashboard links are reachable and panel data aligns with SLO contracts in `docs/observability-stack-phase1.md`.
2. Alert drill evidence is current in `docs/observability-alert-drills.md`.
3. Request/job/push correlation can be demonstrated using a shared `request_id` during incident response.
