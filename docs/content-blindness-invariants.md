# Content Blindness Invariants (Phase I)

- Last Updated: 2026-02-17
- Scope: Issue #149 (`CB-003`)

These invariants are mandatory for server-blind assistant content boundaries.

## 1) Host Runtime Must Not Handle Assistant Plaintext

1. API/worker host runtime may process assistant envelope metadata only.
2. User/assistant message body plaintext must be decrypted and processed only inside enclave runtime.
3. Host persistence for assistant session continuity must use encrypted envelopes only.

## 2) OAuth Connect Token Handling Boundary

1. OAuth authorization-code exchange for Google connect must execute in enclave runtime.
2. Host API must not perform direct token endpoint POSTs for authorization-code exchange.
3. Refresh tokens must not appear in host API process memory during connect completion.

## 3) Job Payload Storage Boundary

1. `jobs.payload_ciphertext` and `dead_letter_jobs.payload_ciphertext` are encrypted-at-write.
2. Unnecessary payload writes must be avoided (metadata-only enqueue when payload is not required).
3. Worker plaintext payload access is limited to leased execution-time reads.

## 4) Redis Reliability Boundary

1. Redis reliability state must stay metadata-only (rate limits, circuit breaker, budget counters).
2. Plaintext LLM response payloads must not be serialized into Redis cache keys/values.

## 5) Logs and Audit Boundary

1. Host logs must not include token-bearing provider payload fragments.
2. Audit metadata is operational-only and subject to sensitive key/value redaction.
3. Error surfaces exposed to host must be deterministic and redact sensitive upstream details.

## 6) CI Guardrails

1. Boundary guard tests in `backend/crates/shared/src/enclave/tests/boundary_guards.rs` must fail on:
   1. host-side OAuth code exchange reintroduction,
   2. plaintext Redis LLM cache serialization,
   3. callback trace payload enqueue reintroduction,
   4. plaintext assistant query contract regressions.
2. `just backend-deep-review` is required before merge for backend-impacting privacy changes.
