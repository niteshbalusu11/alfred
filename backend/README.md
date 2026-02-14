# Alfred Backend (Rust Workspace)

This workspace contains the Alfred iOS v1 backend services.

## Crates

1. `crates/api-server`: REST API aligned with `/api/openapi.yaml` backed by Postgres + `sqlx`.
2. `crates/worker`: scheduler/cron worker loop scaffold.
3. `crates/shared`: shared models and env config.

## Local Infrastructure

From repository root:

```bash
just infra-up
just backend-migrate
```

Default local connection string:

```bash
export DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/alfred
```

## Run Services

From `/Users/niteshchowdharybalusu/Documents/alfred/backend`:

```bash
cargo run -p api-server
```

In a second terminal:

```bash
cargo run -p worker
```

## Notes

1. API handlers are backed by Postgres + `sqlx` for current v1 endpoints.
2. Migrations are stored under `/Users/niteshchowdharybalusu/Documents/alfred/db/migrations`.
3. Worker execution remains placeholder logic while durable job processing is implemented.
4. Scalability boundary: DB queries live in `/Users/niteshchowdharybalusu/Documents/alfred/backend/crates/shared/src/repos`, and HTTP code lives in `/Users/niteshchowdharybalusu/Documents/alfred/backend/crates/api-server/src/http.rs`.
