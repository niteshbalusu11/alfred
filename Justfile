set shell := ["bash", "-cu"]

project_root := `pwd`
ios_project := "alfred/alfred.xcodeproj"
ios_scheme := "alfred"
ios_package_dir := "alfred/Packages/AlfredAPIClient"
backend_dir := "backend"

default:
  @just --list

# Validate required local tooling.
check-tools:
  @command -v xcodebuild >/dev/null || (echo "xcodebuild not found" && exit 1)
  @command -v cargo >/dev/null || (echo "cargo not found" && exit 1)
  @command -v swift >/dev/null || (echo "swift not found" && exit 1)
  @echo "Tools OK: xcodebuild, cargo, swift"

# Validate local infrastructure tooling.
check-infra-tools:
  @command -v docker >/dev/null || (echo "docker not found" && exit 1)
  @docker compose version >/dev/null || (echo "docker compose not found" && exit 1)
  @echo "Infra tools OK: docker, docker compose"

# Open the iOS project in Xcode.
ios-open:
  open {{ios_project}}

# Build iOS app for simulator.
ios-build:
  xcodebuild -project {{ios_project}} -scheme {{ios_scheme}} -destination 'generic/platform=iOS Simulator' build

# Run iOS tests on a specific simulator.
ios-test destination='platform=iOS Simulator,name=iPhone 17':
  xcodebuild -project {{ios_project}} -scheme {{ios_scheme}} -destination '{{destination}}' test

# Compile the local Swift package used by the iOS app.
ios-package-build:
  cd {{ios_package_dir}} && swift build

# Run Rust backend compile checks.
backend-check:
  cd {{backend_dir}} && cargo check

# Start local infrastructure (Postgres).
infra-up:
  docker compose up -d postgres
  @echo "Postgres is starting on 127.0.0.1:5432 (DB: alfred, user: postgres)."
  @echo "Export DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/alfred"

# Stop local infrastructure without deleting volumes.
infra-stop:
  docker compose stop postgres

# Stop and remove local infrastructure including volumes.
infra-down:
  docker compose down -v --remove-orphans

# Tail local infrastructure logs.
infra-logs:
  docker compose logs -f postgres

# Build Rust backend workspace.
backend-build:
  cd {{backend_dir}} && cargo build --workspace

# Run Rust backend tests.
backend-test:
  cd {{backend_dir}} && cargo test

# Install sqlx-cli locally when missing.
install-sqlx-cli:
  @command -v sqlx >/dev/null || cargo install sqlx-cli --no-default-features --features rustls,postgres

# Apply SQL migrations to the configured Postgres database.
backend-migrate: install-sqlx-cli
  cd {{backend_dir}} && sqlx migrate run --source ../db/migrations

# Show migration state for the configured Postgres database.
backend-migrate-check: install-sqlx-cli
  cd {{backend_dir}} && sqlx migrate info --source ../db/migrations

# Format Rust code.
backend-fmt:
  cd {{backend_dir}} && cargo fmt --all

# Check Rust formatting (CI-safe).
backend-fmt-check:
  cd {{backend_dir}} && cargo fmt --all --check

# Lint Rust code.
backend-clippy:
  cd {{backend_dir}} && cargo clippy --workspace --all-targets -- -D warnings

# Full backend quality gate for task completion.
backend-verify:
  cd {{backend_dir}} && cargo fmt --all
  cd {{backend_dir}} && cargo clippy --workspace --all-targets -- -D warnings
  cd {{backend_dir}} && cargo test
  cd {{backend_dir}} && cargo build --workspace

# CI backend gate (non-mutating).
backend-ci:
  cd {{backend_dir}} && cargo fmt --all --check
  cd {{backend_dir}} && cargo clippy --workspace --all-targets -- -D warnings
  cd {{backend_dir}} && cargo test
  cd {{backend_dir}} && cargo build --workspace

# Security-focused dependency audit for backend.
backend-security-audit:
  @command -v cargo-audit >/dev/null || cargo install cargo-audit
  cd {{backend_dir}} && cargo audit --ignore RUSTSEC-2023-0071

# Bug-focused checks for backend code quality.
backend-bug-check:
  cd {{backend_dir}} && cargo test
  @if rg -n "todo!\\(|unimplemented!\\(|dbg!\\(|panic!\\(" {{backend_dir}}/crates; then \
    echo "Bug check failed: remove debug or placeholder macros (todo!/unimplemented!/dbg!/panic!)."; \
    exit 1; \
  fi

# Enforce architecture boundaries for scalability.
backend-architecture-check:
  @if rg -n "sqlx::query|sqlx::query_as|sqlx::query_scalar" {{backend_dir}}/crates/api-server/src; then \
    echo "Architecture violation: SQL queries must stay in backend/crates/shared/src/repos."; \
    exit 1; \
  fi
  @if rg -n "axum::|Router|StatusCode|Json\\(" {{backend_dir}}/crates/shared/src/repos; then \
    echo "Architecture violation: HTTP concerns must stay in backend/crates/api-server/src/http.rs."; \
    exit 1; \
  fi

# Mandatory deep review gate after backend issue implementation.
backend-deep-review:
  just backend-verify
  just backend-security-audit
  just backend-bug-check
  just backend-architecture-check

# Run API server.
backend-api:
  cd {{backend_dir}} && \
    DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@127.0.0.1:5432/alfred}" \
    DATA_ENCRYPTION_KEY="${DATA_ENCRYPTION_KEY:-dev-only-change-me}" \
    GOOGLE_OAUTH_CLIENT_ID="${GOOGLE_OAUTH_CLIENT_ID:-dev-client-id}" \
    GOOGLE_OAUTH_CLIENT_SECRET="${GOOGLE_OAUTH_CLIENT_SECRET:-dev-client-secret}" \
    GOOGLE_OAUTH_REDIRECT_URI="${GOOGLE_OAUTH_REDIRECT_URI:-http://localhost/oauth/callback}" \
    cargo run -p api-server

# Run background worker.
backend-worker:
  cd {{backend_dir}} && \
    DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@127.0.0.1:5432/alfred}" \
    DATA_ENCRYPTION_KEY="${DATA_ENCRYPTION_KEY:-dev-only-change-me}" \
    cargo run -p worker

# Run API and worker together in one terminal session.
dev:
  @trap 'kill 0' INT TERM EXIT; \
    (cd {{backend_dir}} && \
      DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@127.0.0.1:5432/alfred}" \
      DATA_ENCRYPTION_KEY="${DATA_ENCRYPTION_KEY:-dev-only-change-me}" \
      GOOGLE_OAUTH_CLIENT_ID="${GOOGLE_OAUTH_CLIENT_ID:-dev-client-id}" \
      GOOGLE_OAUTH_CLIENT_SECRET="${GOOGLE_OAUTH_CLIENT_SECRET:-dev-client-secret}" \
      GOOGLE_OAUTH_REDIRECT_URI="${GOOGLE_OAUTH_REDIRECT_URI:-http://localhost/oauth/callback}" \
      cargo run -p api-server) & \
    (cd {{backend_dir}} && \
      DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@127.0.0.1:5432/alfred}" \
      DATA_ENCRYPTION_KEY="${DATA_ENCRYPTION_KEY:-dev-only-change-me}" \
      cargo run -p worker) & \
    wait

# Show key project docs.
docs:
  @echo "RFC:      {{project_root}}/docs/rfc-0001-alfred-ios-v1.md"
  @echo "OpenAPI:  {{project_root}}/api/openapi.yaml"
  @echo "DB SQL:   {{project_root}}/db/migrations"
  @echo "Backend:  {{project_root}}/backend/README.md"

# After PR merge: sync local branch to latest master.
sync-master:
  git fetch origin
  git checkout master
  git pull --ff-only origin master
