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

# Open the iOS project in Xcode.
ios-open:
  open {{ios_project}}

# Build iOS app for simulator.
ios-build:
  xcodebuild -project {{ios_project}} -scheme {{ios_scheme}} -destination 'generic/platform=iOS Simulator' build

# Run iOS tests on a specific simulator.
ios-test destination='platform=iOS Simulator,name=iPhone 16':
  xcodebuild -project {{ios_project}} -scheme {{ios_scheme}} -destination '{{destination}}' test

# Compile the local Swift package used by the iOS app.
ios-package-build:
  cd {{ios_package_dir}} && swift build

# Run Rust backend compile checks.
backend-check:
  cd {{backend_dir}} && cargo check

# Build Rust backend workspace.
backend-build:
  cd {{backend_dir}} && cargo build --workspace

# Run Rust backend tests.
backend-test:
  cd {{backend_dir}} && cargo test

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

# Run API server.
backend-api:
  cd {{backend_dir}} && cargo run -p api-server

# Run background worker.
backend-worker:
  cd {{backend_dir}} && cargo run -p worker

# Run API and worker together in one terminal session.
dev:
  @trap 'kill 0' INT TERM EXIT; \
    (cd {{backend_dir}} && cargo run -p api-server) & \
    (cd {{backend_dir}} && cargo run -p worker) & \
    wait

# Show key project docs.
docs:
  @echo "RFC:      {{project_root}}/docs/rfc-0001-alfred-ios-v1.md"
  @echo "OpenAPI:  {{project_root}}/api/openapi.yaml"
  @echo "DB SQL:   {{project_root}}/db/migrations/0001_init.sql"
  @echo "Backend:  {{project_root}}/backend/README.md"

# After PR merge: sync local branch to latest master.
sync-master:
  git fetch origin
  git checkout master
  git pull --ff-only origin master
