# Alfred Backend (Rust Workspace)

This workspace contains the Alfred iOS v1 backend scaffold.

## Crates

1. `crates/api-server`: REST API aligned with `/api/openapi.yaml` (stub handlers).
2. `crates/worker`: scheduler/cron worker loop scaffold.
3. `crates/shared`: shared models and env config.

## Run

From `/Users/niteshchowdharybalusu/Documents/alfred/backend`:

```bash
cargo run -p api-server
```

In a second terminal:

```bash
cargo run -p worker
```

## Notes

1. Handlers are intentionally stubbed for fast wiring with iOS.
2. Next implementation step is adding Postgres + sqlx and wiring real persistence.
3. SQL schema draft is in `/Users/niteshchowdharybalusu/Documents/alfred/db/migrations/0001_init.sql`.
