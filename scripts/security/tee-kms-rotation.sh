#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/security/tee-kms-rotation.sh <preflight|stage|validate|rollback|all> --env <staging|production> [options]

Options:
  --dry-run                 Print actions without executing remote/operator commands.
  --evidence-dir <path>     Directory for execution evidence logs (default: artifacts/security/rotation).
  --confirm-production      Required when --env production is used.
  -h, --help                Show this help text.

Required environment variables:
  KMS_KEY_ID
  KMS_KEY_VERSION
  KMS_ALLOWED_MEASUREMENTS

Stage command variables:
  ALFRED_ROTATION_STAGE_CMD       Required for stage/all (non-dry-run).
  NEXT_KMS_KEY_VERSION            Required for stage/all.

Validation command variables:
  ALFRED_ROTATION_VALIDATE_CMD    Required for validate/all (non-dry-run).

Rollback command variables:
  ALFRED_ROTATION_ROLLBACK_CMD    Required for rollback.

Optional command variables:
  ALFRED_ROTATION_PREFLIGHT_CMD

Optional healthcheck URLs:
  ALFRED_ROTATION_API_HEALTHCHECK_URL
  ALFRED_ROTATION_ENCLAVE_HEALTHCHECK_URL
USAGE
}

if [[ $# -lt 1 ]]; then
  usage
  exit 1
fi

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

ACTION="$1"
shift

TARGET_ENV=""
DRY_RUN=false
EVIDENCE_DIR="artifacts/security/rotation"
CONFIRM_PRODUCTION=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --env)
      TARGET_ENV="${2:-}"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=true
      shift
      ;;
    --evidence-dir)
      EVIDENCE_DIR="${2:-}"
      shift 2
      ;;
    --confirm-production)
      CONFIRM_PRODUCTION=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage
      exit 1
      ;;
  esac
done

case "$ACTION" in
  preflight|stage|validate|rollback|all) ;;
  *)
    echo "Invalid action: $ACTION" >&2
    usage
    exit 1
    ;;
esac

if [[ -z "$TARGET_ENV" ]]; then
  echo "Missing required argument: --env <staging|production>" >&2
  usage
  exit 1
fi

if [[ "$TARGET_ENV" != "staging" && "$TARGET_ENV" != "production" ]]; then
  echo "Invalid environment target: $TARGET_ENV" >&2
  exit 1
fi

if [[ "$TARGET_ENV" == "production" && "$CONFIRM_PRODUCTION" != true ]]; then
  echo "Production execution requires --confirm-production." >&2
  exit 1
fi

if [[ -n "${ALFRED_ENV:-}" && "$ALFRED_ENV" != "$TARGET_ENV" ]]; then
  echo "ALFRED_ENV ($ALFRED_ENV) does not match --env $TARGET_ENV" >&2
  exit 1
fi

TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "$EVIDENCE_DIR"
EVIDENCE_FILE="$EVIDENCE_DIR/${TARGET_ENV}-${ACTION}-${TIMESTAMP}.log"
touch "$EVIDENCE_FILE"

log() {
  local message="$1"
  printf '[%s] %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$message" | tee -a "$EVIDENCE_FILE"
}

require_env_var() {
  local var_name="$1"
  if [[ -z "${!var_name:-}" ]]; then
    log "ERROR missing required env var: $var_name"
    exit 1
  fi
}

run_command() {
  local label="$1"
  local cmd="$2"

  if [[ -z "$cmd" ]]; then
    log "ERROR command missing for step: $label"
    exit 1
  fi

  if [[ "$DRY_RUN" == true ]]; then
    log "DRY-RUN $label -> $cmd"
    return
  fi

  log "RUN $label"
  bash -lc "$cmd" 2>&1 | tee -a "$EVIDENCE_FILE"
}

run_optional_command() {
  local label="$1"
  local cmd="$2"

  if [[ -z "$cmd" ]]; then
    log "SKIP $label (no command configured)"
    return
  fi

  run_command "$label" "$cmd"
}

check_health_url_if_set() {
  local label="$1"
  local url="$2"

  if [[ -z "$url" ]]; then
    log "SKIP $label (no URL configured)"
    return
  fi

  if [[ "$DRY_RUN" == true ]]; then
    log "DRY-RUN $label -> curl --fail --silent --show-error $url"
    return
  fi

  log "RUN $label -> $url"
  curl --fail --silent --show-error "$url" >/dev/null
  log "OK $label"
}

run_preflight() {
  require_env_var KMS_KEY_ID
  require_env_var KMS_KEY_VERSION
  require_env_var KMS_ALLOWED_MEASUREMENTS

  log "Preflight checks for env=$TARGET_ENV"
  check_health_url_if_set "API healthcheck" "${ALFRED_ROTATION_API_HEALTHCHECK_URL:-}"
  check_health_url_if_set "Enclave healthcheck" "${ALFRED_ROTATION_ENCLAVE_HEALTHCHECK_URL:-}"
  run_optional_command "Custom preflight command" "${ALFRED_ROTATION_PREFLIGHT_CMD:-}"
  log "Preflight completed"
}

run_stage_rotation() {
  require_env_var NEXT_KMS_KEY_VERSION

  if [[ "$NEXT_KMS_KEY_VERSION" == "$KMS_KEY_VERSION" ]]; then
    log "ERROR NEXT_KMS_KEY_VERSION must differ from KMS_KEY_VERSION"
    exit 1
  fi

  log "Staged rotation target: key_id=$KMS_KEY_ID current_version=$KMS_KEY_VERSION next_version=$NEXT_KMS_KEY_VERSION"

  if [[ "$DRY_RUN" != true ]]; then
    require_env_var ALFRED_ROTATION_STAGE_CMD
  fi

  run_command "Staged rotation" "${ALFRED_ROTATION_STAGE_CMD:-echo 'set ALFRED_ROTATION_STAGE_CMD'}"
  log "Staged rotation completed"
}

run_validation() {
  if [[ "$DRY_RUN" != true ]]; then
    require_env_var ALFRED_ROTATION_VALIDATE_CMD
  fi

  run_command "Post-rotation validation" "${ALFRED_ROTATION_VALIDATE_CMD:-echo 'set ALFRED_ROTATION_VALIDATE_CMD'}"
  check_health_url_if_set "Post-rotation API healthcheck" "${ALFRED_ROTATION_API_HEALTHCHECK_URL:-}"
  check_health_url_if_set "Post-rotation enclave healthcheck" "${ALFRED_ROTATION_ENCLAVE_HEALTHCHECK_URL:-}"
  log "Validation completed"
}

run_rollback() {
  if [[ "$DRY_RUN" != true ]]; then
    require_env_var ALFRED_ROTATION_ROLLBACK_CMD
  fi

  run_command "Rollback" "${ALFRED_ROTATION_ROLLBACK_CMD:-echo 'set ALFRED_ROTATION_ROLLBACK_CMD'}"
  check_health_url_if_set "Post-rollback API healthcheck" "${ALFRED_ROTATION_API_HEALTHCHECK_URL:-}"
  check_health_url_if_set "Post-rollback enclave healthcheck" "${ALFRED_ROTATION_ENCLAVE_HEALTHCHECK_URL:-}"
  log "Rollback completed"
}

require_env_var KMS_KEY_ID
require_env_var KMS_KEY_VERSION
require_env_var KMS_ALLOWED_MEASUREMENTS

log "Starting action=$ACTION env=$TARGET_ENV dry_run=$DRY_RUN evidence_file=$EVIDENCE_FILE"

case "$ACTION" in
  preflight)
    run_preflight
    ;;
  stage)
    run_preflight
    run_stage_rotation
    ;;
  validate)
    run_validation
    ;;
  rollback)
    run_rollback
    ;;
  all)
    run_preflight
    run_stage_rotation
    run_validation
    ;;
esac

log "Action completed successfully"
