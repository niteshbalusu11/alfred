#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

export ALFRED_ENV="${ALFRED_ENV:-local}"
export ENCLAVE_RUNTIME_MODE="${ENCLAVE_RUNTIME_MODE:-dev-shim}"
export ENCLAVE_RUNTIME_BIND_ADDR="${ENCLAVE_RUNTIME_BIND_ADDR:-127.0.0.1:8181}"
export TEE_ATTESTATION_REQUIRED="${TEE_ATTESTATION_REQUIRED:-false}"
export TEE_ALLOW_INSECURE_DEV_ATTESTATION="${TEE_ALLOW_INSECURE_DEV_ATTESTATION:-true}"

exec just backend-enclave-runtime
