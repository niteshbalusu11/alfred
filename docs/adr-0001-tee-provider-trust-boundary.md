# ADR-0001: Phase I TEE Provider and Trust Boundary Contract

- Status: Accepted
- Date: 2026-02-16
- Owners: Security + Backend
- Related Issues: #121, #122, #123, #124, #125, #126, #127, #130, #43

## 1) Context

Phase I requires a production-real trusted execution boundary for connector token decrypt/refresh/revoke and token-backed Google operations. Existing docs describe target controls, but SEC-001 requires one canonical architecture decision that follow-on implementation issues can treat as source of truth.

Without a fixed provider/runtime and a concrete trust contract, later SEC issues can drift in attestation format, KMS policy assumptions, and host/enclave responsibilities.

## 2) Decision

### 2.1 TEE Provider and Runtime

1. Production runtime for Phase I is AWS Nitro Enclaves.
2. Enclave process is a distinct deployable boundary from host API/worker runtimes.
3. Non-production environments may use a local dev shim only under explicit insecure-dev controls defined in this ADR.

### 2.2 Trust Boundaries

1. Boundary A (Host Runtime): API/worker hosts are untrusted for connector-token plaintext and must not perform direct decrypt operations.
2. Boundary B (Attested Enclave): Enclave is trusted for sensitive operations only after successful challenge-bound attestation verification and allow-list checks.
3. Boundary C (KMS Policy): KMS decrypt authorization must be bound to approved attested enclave identity and connector key metadata (`token_key_id`, `token_version`).
4. Boundary D (Provider Fetch): Token-backed Google fetch/refresh/revoke operations must execute only inside enclave handlers once authorization checks pass.

### 2.3 Attestation Contract (Challenge-Based, Fail-Closed)

Host must issue a challenge for every sensitive operation session:

1. `challenge_nonce` (cryptographically random, single-use).
2. `issued_at` and `expires_at` (freshness window).
3. `operation_purpose` (for example: decrypt, refresh, revoke, fetch).
4. Correlation identifier (`request_id`) for auditability.

Enclave must return signed attestation evidence bound to the challenge:

1. Evidence document signed by runtime trust chain.
2. Echoed `challenge_nonce`.
3. Enclave runtime identity and measurement fields required by verifier allow-list.
4. Evidence timestamp data.

Verification requirements:

1. Signature and certificate chain validation succeeds.
2. Challenge nonce matches and is not replayed.
3. Runtime type and measurements are in configured allow-lists.
4. Evidence freshness is within allowed TTL.
5. Any validation failure denies authorization.

### 2.4 KMS Binding Contract

1. Decrypt operations require KMS policy conditions tied to approved enclave identity attributes.
2. Decrypt request context must include connector key metadata at minimum:
   1. `token_key_id`
   2. `token_version`
3. Stored metadata and request context must match KMS policy constraints.
4. Host process credentials must not have direct decrypt capability for connector token paths.
5. Policy mismatch, missing metadata, or disallowed enclave identity must fail closed.

### 2.5 Production vs Local-Dev Behavior

Production and staging:

1. Insecure attestation bypass modes are forbidden.
2. Missing attestation evidence or verifier config must fail startup or fail request authorization.
3. Host direct token decrypt/fetch paths are forbidden.

Local development:

1. Dev shim is allowed only with explicit opt-in (`ALFRED_ENCLAVE_MODE=dev-shim` and `ALFRED_ALLOW_INSECURE_ATTESTATION=true`).
2. Dev shim mode must be clearly marked in logs and startup output.
3. Dev shim mode is blocked in production/staging profiles.
4. Security-sensitive tests must include real fail-closed behavior checks independent of dev shim.

## 3) Required Security and Reliability Properties

1. Fail-closed by default for attestation/KMS mismatch conditions.
2. Deterministic, redacted error mapping from enclave and verifier paths.
3. Replay protection for challenge nonces.
4. No secret values in logs, traces, or error payloads.
5. Audit evidence for allow/deny decisions without exposing secrets.

## 4) Consequences for Follow-On Work

1. SEC-002 implements the deployable enclave runtime and dev harness consistent with this ADR.
2. SEC-003 implements nonce-based attestation verification exactly against this contract.
3. SEC-004 binds KMS policy checks to attested identity plus key metadata contract.
4. SEC-005 implements secure host<->enclave RPC with authn/authz and deterministic error contracts.
5. SEC-006 migrates token-backed Google fetch/decrypt paths into enclave-only execution.
6. SEC-007 and SEC-008 implement key versioning and rotation operationally against this policy model.
7. SEC-009 enforces no-secrets-in-logs guarantees for all sensitive paths.

## 5) Validation Requirements

1. Unit tests for attestation verifier success and each fail-closed branch.
2. Integration tests proving host cannot decrypt without approved enclave identity.
3. Integration tests for replay rejection and freshness window expiry.
4. CI guardrails that fail on secret leakage in sensitive logging paths.

## 6) References

1. `docs/rfc-0001-alfred-ios-v1.md`
2. `docs/threat-model-phase1.md`
3. `docs/iam-least-privilege-review.md`
4. `docs/security-hardening-checklist.md`
