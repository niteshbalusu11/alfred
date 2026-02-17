# Assistant v2 Privacy + Eval Guardrails

- Last Updated: 2026-02-17
- Scope: Assistant query path (`/v1/assistant/query`) with encrypted request/response envelopes and enclave-only plaintext.

## 1) Privacy Boundary Invariants

1. Host control-plane paths only handle encrypted assistant message envelopes.
2. Plaintext assistant query/response content is allowed only inside attested enclave runtime paths.
3. Host persistence in `assistant_encrypted_sessions.state_json` must remain ciphertext-only; plaintext query/response snippets must not be stored.
4. Host assistant error logs must not include upstream enclave `message` body text.
5. API-visible `AssistantQueryResponse` contract must remain envelope-only (`session_id` + `envelope`).

## 2) Eval and Test Coverage Expectations

Assistant v2 is considered guarded only when all of the following pass:

1. `just backend-eval`
2. `just backend-tests`
3. `just backend-verify`
4. `just backend-deep-review`

### `backend-eval` assistant coverage

`backend-eval` now includes deterministic assistant routing fixtures/goldens for:

1. `general_chat`
2. `calendar_lookup`
3. `email_lookup`
4. `mixed`
5. follow-up routing that reuses prior capability context

Fixture path:

- `backend/crates/llm-eval/fixtures/assistant_cases`

Goldens path:

- `backend/crates/llm-eval/fixtures/goldens/assistant_*.golden.json`

## 3) Operational Verification Checklist

Before merge of assistant backend work:

1. Confirm boundary guard tests pass for assistant request/response contracts.
2. Confirm encrypted e2e assistant round-trip test verifies no plaintext leakage in host API payloads and DB session state.
3. Confirm assistant error mapping logs are metadata-only (no upstream message body).
4. Confirm PR includes AI review summary (`docs/ai-review-template.md`) with security/bug/scalability findings.
