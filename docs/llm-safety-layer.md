# LLM Safety Layer (Issue #96)

## Purpose

Define baseline runtime safeguards for all LLM-backed backend capabilities.

## Safety Controls

1. Strict output schema enforcement:
   1. Every model response must validate against typed capability contracts.
   2. Invalid or non-conforming responses are rejected from normal flow.
2. Prompt-injection guard for connector context:
   1. Context payload strings are sanitized before being sent to the model.
   2. Injection-like instruction content is replaced with a redacted placeholder.
3. Deterministic fallback outputs:
   1. If provider output is invalid, missing, or unavailable, backend emits deterministic fallback content.
   2. Fallback outputs stay within the same typed contracts as normal model outputs.
4. Action safety policy checks:
   1. Actionable urgent-email outputs (`should_notify=true`) are only accepted when urgency is `high` or `critical` and rationale/actions are present.
   2. If policy checks fail, deterministic fallback is used and notification is suppressed.

## Current Integration Points

1. Shared safety utilities:
   1. `backend/crates/shared/src/llm/safety.rs`
2. Assistant query endpoint safety path:
   1. `backend/crates/api-server/src/http/assistant/query.rs`
3. Prompt templates include explicit instruction to treat context as untrusted:
   1. `backend/crates/shared/src/llm/prompts.rs`
4. Assistant session follow-up memory path:
   1. `backend/crates/shared/src/repos/assistant_sessions.rs`
   2. `backend/crates/api-server/src/http/assistant/query.rs`
   3. `db/migrations/0009_assistant_sessions.sql`

## Assistant Session Memory Controls (Issue #101)

1. Session continuity uses `session_id` on `POST /v1/assistant/query`.
2. Stored memory is bounded and encrypted:
   1. Sliding TTL: 6 hours.
   2. Max recent turns: 6.
   3. Per-turn snippets are sanitized/truncated before persistence.
3. Persisted payload is minimal redacted memory only:
   1. User query snippet.
   2. Assistant summary snippet.
   3. Capability + timestamp.
4. Raw connector payloads and raw LLM prompt/context payloads are not persisted as memory.
5. Privacy delete purges assistant session rows in `purge_user_operational_data`.

## Tests

1. Safety unit tests live in:
   1. `backend/crates/shared/src/llm/safety.rs` (`#[cfg(test)]` module)
2. Covered scenarios:
   1. Injection-like text redaction.
   2. Valid output passthrough.
   3. Invalid output fallback.
   4. Unsafe actionable urgent-email output fallback.
3. Session memory tests live in:
   1. `backend/crates/api-server/src/http/assistant/memory.rs` (`#[cfg(test)]` module)
4. Covered session-memory scenarios:
   1. Ambiguous follow-up capability reuse from prior session capability.
   2. Bounded turn window and truncation/redaction behavior.
   3. Empty-memory omission from model context.
