#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${1:-${ENCLAVE_RUNTIME_BASE_URL:-http://127.0.0.1:8181}}"
NOW="$(date +%s)"
EXPIRES="$((NOW + 30))"

curl --fail --silent --show-error "$BASE_URL/healthz" >/dev/null
curl --fail --silent --show-error "$BASE_URL/v1/attestation/document" >/dev/null
curl --fail --silent --show-error \
  -H "content-type: application/json" \
  -X POST \
  -d "{\"challenge_nonce\":\"smoke-nonce\",\"issued_at\":${NOW},\"expires_at\":${EXPIRES},\"operation_purpose\":\"smoke\",\"request_id\":\"smoke-request\"}" \
  "$BASE_URL/v1/attestation/challenge" >/dev/null

echo "enclave-runtime smoke checks passed for $BASE_URL"
