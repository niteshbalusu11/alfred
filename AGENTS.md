# Alfred Agent Instructions

This file is intentionally at repository root so coding agents can auto-discover it.

## Start Here

1. Read `/Users/niteshchowdharybalusu/Documents/alfred/agent/start.md` before making changes.
2. Use the `Justfile` at repo root for all common workflows.

## Required Workflow

1. Run `just check-tools`.
2. Run `just backend-check`.
3. Run `just ios-build`.
4. Make scoped changes.
5. Re-run relevant checks before finishing:
   1. `just backend-check`
   2. `just ios-build`
   3. `just backend-test` when backend behavior changes

## Key Paths

1. iOS app: `/Users/niteshchowdharybalusu/Documents/alfred/alfred`
2. iOS API package: `/Users/niteshchowdharybalusu/Documents/alfred/alfred/Packages/AlfredAPIClient`
3. Backend workspace: `/Users/niteshchowdharybalusu/Documents/alfred/backend`
4. OpenAPI contract: `/Users/niteshchowdharybalusu/Documents/alfred/api/openapi.yaml`
5. DB migrations: `/Users/niteshchowdharybalusu/Documents/alfred/db/migrations`

## Guardrails

1. Keep API/server/client contracts aligned.
2. Use migrations for any schema changes.
3. Do not weaken privacy constraints (token/data handling) without explicit approval.
