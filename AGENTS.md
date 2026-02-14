# Alfred Agent Instructions

This file is intentionally at repository root so coding agents can auto-discover it.

## Start Here

1. Read `/Users/niteshchowdharybalusu/Documents/alfred/agent/start.md` before making changes.
2. Use the `Justfile` at repo root for all common workflows.
3. Work from GitHub issues first, then keep the Phase I board aligned.

## Required Workflow

1. Run `just check-tools`.
2. Run `just backend-check`.
3. Run `just ios-build`.
4. Make scoped changes.
5. Re-run relevant checks before finishing:
   1. `just backend-check`
   2. `just ios-build`
   3. `just backend-test` when backend behavior changes

## Planning and Issue Source of Truth

1. Execution queue: GitHub issues in `niteshbalusu11/alfred` with labels:
   1. `phase-1`
   2. `P0` or `P1`
2. Planning board: `/Users/niteshchowdharybalusu/Documents/alfred/docs/phase1-master-todo.md`
3. Rule:
   1. If a GitHub issue and board item conflict, treat GitHub issue as immediate execution source and update docs in the same change.

## GitHub Issue Workflow (Required)

1. Pick next issue in priority order:
   1. `phase-1 + P0` first
   2. Then `phase-1 + P1`
   3. Within same priority, pick lowest issue number unless blocked by dependencies
2. Before coding:
   1. Confirm issue acceptance criteria and dependencies
   2. Update issue comment/status to indicate active work
3. Branching:
   1. Use branch name prefix `codex/`
   2. Recommended format: `codex/issue-<number>-<short-slug>`
4. During implementation:
   1. Keep scope strictly to issue acceptance criteria
   2. If scope must expand, document reason in issue comment first
5. Before handoff:
   1. Run required checks from this file
   2. Summarize what passed/failed
   3. Update issue with completion notes and any follow-ups
6. If blocked:
   1. Add explicit blocker comment to issue
   2. Move to next unblocked highest-priority issue
7. Use standard issue update format:
   1. `/Users/niteshchowdharybalusu/Documents/alfred/docs/issue-update-template.md`

## Key Paths

1. iOS app: `/Users/niteshchowdharybalusu/Documents/alfred/alfred`
2. iOS API package: `/Users/niteshchowdharybalusu/Documents/alfred/alfred/Packages/AlfredAPIClient`
3. Backend workspace: `/Users/niteshchowdharybalusu/Documents/alfred/backend`
4. OpenAPI contract: `/Users/niteshchowdharybalusu/Documents/alfred/api/openapi.yaml`
5. DB migrations: `/Users/niteshchowdharybalusu/Documents/alfred/db/migrations`
6. Phase I board: `/Users/niteshchowdharybalusu/Documents/alfred/docs/phase1-master-todo.md`

## Guardrails

1. Keep API/server/client contracts aligned.
2. Use migrations for any schema changes.
3. Do not weaken privacy constraints (token/data handling) without explicit approval.
4. Do not implement work outside an active GitHub issue unless explicitly requested.
