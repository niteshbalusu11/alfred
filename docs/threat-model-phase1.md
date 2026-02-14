# Phase I STRIDE Threat Model

- Last Updated: 2026-02-14
- Reviewed For Issue: `#9`
- Scope: iOS session auth, OAuth connector lifecycle, TEE decrypt path, worker processing, privacy delete flow

## 1) System Boundaries

1. iOS client calls API endpoints over TLS.
2. API server writes/reads operational state in Postgres.
3. Worker executes scheduled jobs and provider fetches.
4. Enclave + KMS path is required for refresh-token decrypt.
5. APNs is used for outbound user notifications.

## 2) Critical Assets

1. Connector refresh token ciphertext and key metadata.
2. iOS access/refresh session tokens and hashed lookup keys.
3. User PII and preferences persisted in backend tables.
4. Audit event stream proving security-sensitive actions.
5. Attestation documents, allowed measurements, and KMS key policy bindings.

## 3) STRIDE Analysis

| Category | Threat | Impact | Current Mitigations | Residual Risk |
|---|---|---|---|---|
| Spoofing | Forged session bearer token or stolen OAuth state | Unauthorized account access or connector hijack | Hashed token lookup, OAuth state TTL + one-time consumption, auth middleware rejects invalid bearer | Medium: relies on client token handling hygiene |
| Tampering | DB row mutation of connector key metadata or status | Incorrect decrypt policy path, revoke bypass | Strict key-id/key-version checks, ACTIVE status checks, migration-backed schema | Medium: requires tight DB IAM + change auditing |
| Repudiation | Actor denies issuing revoke/delete-all/security-sensitive actions | Weak incident forensics and support evidence | Audit events for session/connect/revoke/delete lifecycle; deterministic API outcomes | Low: enrich actor metadata over time |
| Information Disclosure | Token/plaintext leakage via logs/errors or host decrypt path | Credential compromise and privacy breach | Redacted errors, no plaintext token logs, enclave/attestation gating for decrypt | Medium: guardrails must remain tested in CI |
| Denial of Service | Endpoint abuse (auth/connect/delete) and retry storms | Service instability or user lockouts | Endpoint rate limiting for sensitive routes; deterministic 429 + Retry-After; worker lease/retry controls | Medium: distributed abuse needs upstream WAF controls too |
| Elevation of Privilege | Host process or compromised runtime bypasses enclave constraints | Decrypt outside trusted boundary | Attestation verification + measurement allow-lists + KMS-bound policy; fail-closed checks | Medium: depends on secure key and attestation ops |

## 4) Threat-Specific Controls Added in This Pass

1. Endpoint rate limits now enforce quotas on:
   1. `POST /v1/auth/ios/session`
   2. `POST /v1/connectors/google/start`
   3. `POST /v1/connectors/google/callback`
   4. `DELETE /v1/connectors/{connector_id}`
   5. `POST /v1/privacy/delete-all`
2. Secret-scanning is required in CI and blocks merge on detected leaks.
3. IAM least-privilege evidence is documented in `docs/iam-least-privilege-review.md`.
4. Security hardening checklist completion is tracked in `docs/security-hardening-checklist.md`.

## 5) Review Outcome

1. No unresolved critical risks were identified in this pass.
2. Remaining medium risks are operational and tracked for follow-up hardening:
   1. Upstream WAF/IP reputation controls for distributed abuse patterns.
   2. Periodic attestation/KMS policy drift review.
   3. Automated verification that log redaction coverage stays complete.
