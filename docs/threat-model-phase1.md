# Phase I STRIDE Threat Model

- Last Updated: 2026-02-16
- Reviewed For Issue: `#19`
- Scope: iOS session auth, OAuth connector lifecycle, TEE decrypt path, worker processing, LLM/OpenRouter orchestration, privacy delete flow

Canonical TEE provider/trust contract reference:
`docs/adr-0001-tee-provider-trust-boundary.md` (SEC-001)

## 1) System Boundaries

1. iOS client calls API endpoints over TLS.
2. API server writes/reads operational state in Postgres.
3. Worker executes scheduled jobs and provider fetches.
4. Enclave RPC + KMS path is required for refresh-token decrypt and Google token refresh/revoke operations.
5. APNs is used for outbound user notifications.

## 2) Critical Assets

1. Connector refresh token ciphertext and key metadata.
2. Clerk-issued bearer tokens used for protected API authorization.
3. User PII and preferences persisted in backend tables.
4. Audit event stream proving security-sensitive actions.
5. Attestation documents, allowed measurements, and KMS key policy bindings.
6. LLM provider credentials, model-routing policy, and assistant output safety controls.

## 3) STRIDE Analysis

| Category | Threat | Impact | Current Mitigations | Residual Risk |
|---|---|---|---|---|
| Spoofing | Forged Clerk bearer token or stolen OAuth state | Unauthorized account access or connector hijack | Clerk JWT verification (JWKS + issuer + audience + expiry), OAuth state TTL + one-time consumption | Medium: relies on client token handling hygiene |
| Tampering | DB row mutation of connector key metadata or status | Incorrect decrypt policy path, revoke bypass | Strict key-id/key-version checks, ACTIVE status checks, migration-backed schema | Medium: requires tight DB IAM + change auditing |
| Repudiation | Actor denies issuing revoke/delete-all/security-sensitive actions | Weak incident forensics and support evidence | Audit events for session/connect/revoke/delete lifecycle; deterministic API outcomes | Low: enrich actor metadata over time |
| Information Disclosure | Token/plaintext leakage via logs/errors or host decrypt path | Credential compromise and privacy breach | Redacted errors, no plaintext token logs, enclave RPC-only decrypt/refresh/revoke path, attestation/KMS gating | Low-Medium: guardrails must remain tested in CI |
| Denial of Service | Endpoint abuse (auth/connect/delete) and retry storms; LLM provider degradation | Service instability or user lockouts | Endpoint rate limiting for sensitive routes; deterministic 429 + Retry-After; worker lease/retry controls; provider fallback + deterministic assistant fallback | Medium: distributed abuse needs upstream WAF controls too |
| Elevation of Privilege | Host process or compromised runtime bypasses enclave constraints | Decrypt outside trusted boundary | Compile-time host decrypt removal, enclave RPC contract, attestation verification + measurement allow-lists + KMS-bound policy; fail-closed checks | Medium: depends on secure key and attestation ops |

## 4) Threat-Specific Controls Added in This Pass

1. Endpoint rate limits now enforce quotas on:
   1. `POST /v1/connectors/google/start`
   2. `POST /v1/connectors/google/callback`
   3. `DELETE /v1/connectors/{connector_id}`
   4. `POST /v1/privacy/delete-all`
2. Secret-scanning is required in CI and blocks merge on detected leaks.
3. IAM least-privilege evidence is documented in `docs/iam-least-privilege-review.md`.
4. Security hardening checklist completion is tracked in `docs/security-hardening-checklist.md`.
5. Host direct decrypt path was removed from API/worker and replaced with enclave RPC operations for token refresh/revoke.
6. Automated source-guard test verifies sensitive API/worker tracing macros do not log token/secret fields in enclave-sensitive paths.

## 5) Review Outcome

1. No unresolved critical risks were identified in this pass.
2. Remaining medium risks are operational and tracked for follow-up hardening:
   1. Upstream WAF/IP reputation controls for distributed abuse patterns.
   2. Periodic attestation/KMS policy drift review.
   3. Automated verification that log redaction coverage stays complete.
