# LLM Eval Harness (Issue #102)

This document defines how to run and interpret the backend LLM eval/regression harness.

## Scope

The harness covers representative assistant capabilities:

1. Meetings summary
2. Morning brief
3. Urgent email prioritization

Checks are fixture-driven and deterministic in mocked mode.

## Commands

Run from repository root:

1. `just backend-eval`
   1. Deterministic mocked-mode checks (CI-blocking mode).
2. `just backend-eval-update`
   1. Intentionally refresh goldens after approved prompt/contract/safety changes.
3. `just backend-eval-live`
   1. Optional OpenRouter smoke mode (not CI-blocking).

## Failure Categories

1. `schema_validity`
   1. The model output no longer validates against the typed output contract.
2. `safe_output_source`
   1. Safety policy changed behavior (for example model output is rejected and fallback is used).
3. `quality`
   1. Output quality dropped below baseline checks (empty/weak summaries or actions).
4. `golden_snapshot`
   1. Deterministic request/output snapshot changed from reviewed baseline.

## Golden Update Workflow

Use this flow only for intentional behavior changes:

1. Confirm change is expected and reviewed.
2. Run `just backend-eval-update`.
3. Inspect diffs under `backend/crates/llm-eval/fixtures/goldens`.
4. Include rationale in PR/issue notes for why snapshots changed.

## Live Smoke Mode Notes

1. Requires valid `OPENROUTER_*` environment variables.
2. Executes representative fixtures against a live provider.
3. Validates schema/safety/quality, but does not compare golden snapshots.
4. Intended for optional provider sanity checks; deterministic mocked mode remains source of truth in CI.
