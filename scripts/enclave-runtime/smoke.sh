#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${1:-${ENCLAVE_RUNTIME_BASE_URL:-http://127.0.0.1:8181}}"

curl --fail --silent --show-error "$BASE_URL/healthz" >/dev/null
curl --fail --silent --show-error "$BASE_URL/v1/attestation/document" >/dev/null

echo "enclave-runtime smoke checks passed for $BASE_URL"
