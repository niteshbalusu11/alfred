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

## Tests

1. Safety unit tests live in:
   1. `backend/crates/shared/src/llm/safety.rs` (`#[cfg(test)]` module)
2. Covered scenarios:
   1. Injection-like text redaction.
   2. Valid output passthrough.
   3. Invalid output fallback.
   4. Unsafe actionable urgent-email output fallback.
