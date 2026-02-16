# TEE and KMS Rotation Runbook (SEC-008)

- Last Updated: 2026-02-16
- Owner: SEC
- Related Issues: `#128`, `#130`

## Purpose

Provide a repeatable, fail-safe process for rotating token crypto key versions and enclave measurement allow-lists with explicit validation and rollback checkpoints.

## Scope

1. Token key version rotation execution.
2. Enclave measurement allow-list update coordination.
3. Post-rotation validation and evidence capture.
4. Emergency rollback workflow.

## Operator Inputs

Required environment variables:

1. `KMS_KEY_ID`
2. `KMS_KEY_VERSION`
3. `NEXT_KMS_KEY_VERSION`
4. `KMS_ALLOWED_MEASUREMENTS`

Required command hooks:

1. `ALFRED_ROTATION_STAGE_CMD`
2. `ALFRED_ROTATION_VALIDATE_CMD`
3. `ALFRED_ROTATION_ROLLBACK_CMD`

Optional command hooks:

1. `ALFRED_ROTATION_PREFLIGHT_CMD`

Optional health checks:

1. `ALFRED_ROTATION_API_HEALTHCHECK_URL`
2. `ALFRED_ROTATION_ENCLAVE_HEALTHCHECK_URL`

## Script Interface

Use `scripts/security/tee-kms-rotation.sh`.

Supported actions:

1. `preflight`: validates environment and baseline checks.
2. `stage`: runs preflight and staged rotation command.
3. `validate`: runs post-rotation validation checks.
4. `rollback`: executes emergency rollback command.
5. `all`: runs `preflight -> stage -> validate`.

Required flags:

1. `--env <staging|production>`
2. Use `--confirm-production` for any production execution.

Safety flags:

1. `--dry-run` prints all commands without execution.
2. `--evidence-dir <path>` stores execution logs for audit evidence.
3. `--show-commands` prints full command text in dry-run logs (default behavior hides command text to reduce accidental secret exposure).

## Standard Procedure

1. Preflight

```bash
scripts/security/tee-kms-rotation.sh preflight --env staging --dry-run
```

2. Staged rotation (staging first)

```bash
scripts/security/tee-kms-rotation.sh all --env staging --evidence-dir artifacts/security/rotation
```

3. Production rollout (after staging validation)

```bash
scripts/security/tee-kms-rotation.sh all --env production --confirm-production --evidence-dir artifacts/security/rotation
```

4. Emergency rollback

```bash
scripts/security/tee-kms-rotation.sh rollback --env production --confirm-production --evidence-dir artifacts/security/rotation
```

## Validation Checklist

1. API and enclave health checks pass before and after rotation.
2. Token decrypt/refresh/revoke smoke checks pass for attested enclave path.
3. No new secret-redaction violations appear in logs/alerts.
4. Evidence log is archived with environment, key versions, and command outputs.

## Evidence Requirements

Capture and retain:

1. Script output log from `--evidence-dir`.
2. Rotation command output confirming key version transition.
3. Validation output confirming attestation/KMS policy enforcement.
4. Rollback output (if executed) and post-rollback health checks.

## Incident Integration

For incident triage and escalation, use:

1. `docs/observability-incident-runbook.md`
2. `docs/enclave-runtime-baseline.md`

Record links to evidence artifacts in the relevant incident ticket or GitHub issue comment.
