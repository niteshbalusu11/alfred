# Privacy Delete SLA Monitoring

This runbook defines the baseline dashboard and alert for the Phase I delete-all SLA.

## SLA Target

- Privacy delete request completion within `24 hours` of request creation.

## Dashboard Queries

Use `privacy_delete_requests` as the source table.

```sql
-- Pending and running backlog
SELECT status, COUNT(*) AS request_count
FROM privacy_delete_requests
GROUP BY status
ORDER BY status;
```

```sql
-- SLA breaches (not completed within 24 hours)
SELECT COUNT(*) AS overdue_requests
FROM privacy_delete_requests
WHERE status <> 'COMPLETED'
  AND created_at <= NOW() - INTERVAL '24 hours';
```

```sql
-- End-to-end completion latency distribution
SELECT
  DATE_TRUNC('hour', completed_at) AS completed_hour,
  PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY EXTRACT(EPOCH FROM (completed_at - created_at))) AS p50_seconds,
  PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY EXTRACT(EPOCH FROM (completed_at - created_at))) AS p95_seconds
FROM privacy_delete_requests
WHERE status = 'COMPLETED'
GROUP BY completed_hour
ORDER BY completed_hour DESC;
```

## Alert Rule

- Name: `privacy-delete-sla-breach`
- Condition: `overdue_requests > 0` for `10 minutes`
- Severity: high
- Primary signal:
  - Worker warning log: `privacy delete SLA alert threshold reached`
  - Fields: `overdue_requests`, `sla_hours`, `worker_id`
