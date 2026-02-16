# Observability Stack (Phase I)

This document defines the baseline observability stack for API, worker, and push delivery flows.

## SLOs

| SLI | Target | Window | Source |
|---|---|---|---|
| API availability (`2xx/3xx` success ratio) | >= 99.9% | rolling 7 days | `api_http_request` log events |
| API p95 latency | <= 500ms | rolling 1 hour | `api_http_request.latency_ms` |
| Job processing success rate | >= 99.0% | rolling 24 hours | `worker tick metrics.success_rate` |
| Job lag p95 | <= 60s | rolling 1 hour | `worker tick metrics.average_lag_seconds`, `max_lag_seconds` |
| Push delivery success ratio | >= 98.5% | rolling 24 hours | `worker tick metrics.push_delivered / push_attempts` |
| LLM request success ratio | >= 98.0% | rolling 1 hour | `metric_name=llm_request` grouped by `outcome` |
| LLM p95 latency | <= 4000ms | rolling 1 hour | `llm_request.latency_ms` |

## Instrumentation Baseline

### API

- Middleware emits `api request metrics` for every request.
- Standard fields:
  - `request_id`
  - `method`
  - `route`
  - `status`
  - `latency_ms`
  - `metric_name=api_http_request`
- Response header includes `x-request-id` for client-visible correlation.

### Worker

- Existing `worker tick metrics` event remains the primary aggregate metric source:
  - `claimed_jobs`, `processed_jobs`, `successful_jobs`
  - `retryable_failures`, `permanent_failures`, `dead_lettered_jobs`
  - `push_attempts`, `push_delivered`, `push_transient_failures`, `push_permanent_failures`
  - `average_lag_seconds`, `max_lag_seconds`, `success_rate`
- Request-to-job correlation is propagated through payload trace metadata and surfaced in worker audit metadata/log fields as `request_id`.

### AI / LLM

- API and worker LLM paths emit `metric_name=llm_request`.
- Standard fields:
  - `source` (`api_assistant_query`, `worker_morning_brief`, `worker_urgent_email`)
  - `capability`
  - `outcome` (`success` or `failure`)
  - `provider`, `model`
  - `latency_ms`
  - `prompt_tokens`, `completion_tokens`, `total_tokens` (when provider usage exists)
  - `estimated_cost_usd` (model-price estimate when known)
  - `error_type` (failure-only)
- Sustained provider degradation emits:
  - `event=llm_provider_degradation_alert`
  - `metric_name=llm_provider_degradation`
  - `provider`, `consecutive_failures`, `degraded_for_seconds`
- Recovery after degraded state emits:
  - `event=llm_provider_recovered`
  - `metric_name=llm_provider_degradation`

## Dashboard Links (Staging)

- API Overview: [staging-api-overview](https://grafana.staging.alfred.internal/d/alfred-api-overview)
- Worker Overview: [staging-worker-overview](https://grafana.staging.alfred.internal/d/alfred-worker-overview)
- Push Delivery: [staging-push-overview](https://grafana.staging.alfred.internal/d/alfred-push-overview)
- AI/LLM Overview: [staging-ai-overview](https://grafana.staging.alfred.internal/d/alfred-ai-overview)
- SLO Burn Rates: [staging-slo-burn](https://grafana.staging.alfred.internal/d/alfred-slo-burn)

## Dashboard Panel Contract

This section maps each required SLI/SLO to the expected dashboard panel and metric source.

| Dashboard | Panel | Metric contract | Target/threshold |
|---|---|---|---|
| API Overview | API success ratio | `metric_name=api_http_request` grouped by `status` class (`2xx/3xx` vs total) | >= 99.9% (7d) |
| API Overview | API latency p95 | `api_http_request.latency_ms` p95 grouped by route | <= 500ms (1h) |
| Worker Overview | Job success rate | `worker tick metrics.success_rate` | >= 99.0% (24h) |
| Worker Overview | Queue lag | `worker tick metrics.average_lag_seconds` and `max_lag_seconds` | p95 <= 60s (1h), alert if `max_lag_seconds > 300` |
| Push Delivery | Push success ratio | `worker tick metrics.push_delivered / push_attempts` | >= 98.5% (24h) |
| AI/LLM Overview | LLM success ratio | `metric_name=llm_request` grouped by `outcome` | >= 98.0% (1h) |
| AI/LLM Overview | LLM latency p95 | `llm_request.latency_ms` grouped by `source` and `model` | <= 4000ms (1h) |
| AI/LLM Overview | LLM usage/cost | Sum of `prompt_tokens`, `completion_tokens`, `estimated_cost_usd` by model/provider | Track trend and anomalies |
| SLO Burn Rates | Burn budget | Derived burn series for API availability, worker success rate, and push delivery success | Page on burn breach |

Verification status:
1. SLO definitions documented in this file.
2. Dashboard URLs are documented under `Dashboard Links (Staging)`.
3. Alert drill scenarios and route verification workflow live in `docs/observability-alert-drills.md`.

## Alert Rules

| Alert | Condition | Duration | Severity | Route |
|---|---|---|---|---|
| `alfred-api-availability-burn` | API success ratio below SLO burn threshold | 10m | high | PagerDuty `alfred-primary` + `#alfred-incidents` |
| `alfred-job-lag-high` | `max_lag_seconds > 300` | 10m | high | PagerDuty `alfred-primary` + `#alfred-incidents` |
| `alfred-job-failure-spike` | `permanent_failures` spike over baseline | 10m | high | PagerDuty `alfred-primary` + `#alfred-incidents` |
| `alfred-push-delivery-degraded` | push success ratio below 98.5% | 15m | medium | `#alfred-incidents` |
| `alfred-llm-provider-degraded` | sustained `event=llm_provider_degradation_alert` for any provider | 10m | high | PagerDuty `alfred-primary` + `#alfred-incidents` |

## Correlation Contract

- API request ID source:
  - Accept valid inbound `x-request-id`, otherwise generate UUID v4.
- Worker propagation:
  - API enqueue payload includes `trace.request_id`.
  - Worker extracts `trace.request_id` and writes it into notification audit metadata.
- Operator workflow:
  1. Start from API request logs by `request_id`.
  2. Find queued job and worker audit events with the same `request_id`.
  3. Verify push delivery attempts and outcomes for that request path.

## Alert Drill Procedure

1. Trigger synthetic notification via `POST /v1/devices/apns/test` in staging.
2. Confirm API log event exists with `request_id` and `metric_name=api_http_request`.
3. Force push failure path (staging APNs endpoint disable or auth token mismatch).
4. Confirm worker emits failed delivery attempts and includes `request_id` in metadata/log context.
5. Validate alert route delivery:
   - PagerDuty `alfred-primary`
   - Slack `#alfred-incidents`
6. Resolve incident simulation and confirm alert auto-recovers.

Detailed drill matrix and evidence log:
- `docs/observability-alert-drills.md`

## Runbook

Use `docs/observability-incident-runbook.md` for triage and escalation.
