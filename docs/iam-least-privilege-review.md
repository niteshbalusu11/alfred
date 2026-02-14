# IAM Least-Privilege Review (Phase I)

- Last Updated: 2026-02-14
- Reviewed For Issue: `#9`
- Scope: API server, worker, CI pipeline, data plane dependencies

## 1) Service Identities and Required Permissions

| Service | Required Access | Explicitly Not Required |
|---|---|---|
| `api-server` | Read/write Postgres app tables, call Google OAuth/token/revoke endpoints, enqueue internal jobs | Direct KMS decrypt from host path, broad cloud admin, wildcard secret store reads |
| `worker` | Read/write Postgres app tables, claim/complete job leases, call Google and APNs endpoints | User-management/IAM mutation APIs, raw secret export, broad object storage access |
| `tee-runtime` | Attestation evidence generation, decrypt only for bound key-id/version and approved measurements | Any decrypt outside attested runtime, arbitrary key access |
| `ci` | Repo read + workflow execution + dependency/install operations needed for checks | Production database access, runtime secret retrieval, deployment credential writes |

## 2) Enforcement Expectations

1. Every runtime identity should have a dedicated principal (no shared admin identities).
2. `api-server` and `worker` must use separate IAM roles to limit blast radius.
3. KMS decrypt permissions must be constrained by both key-id and attested measurement conditions.
4. CI should use ephemeral credentials and must not have production data-plane access.
5. Secrets injected to runtime should be scoped per environment and rotated on credential changes.

## 3) Evidence Collected

1. Code path enforces enclave-attested decrypt checks before secret use (`backend/crates/shared/src/security.rs`).
2. API and worker bootstrap only consume env-scoped credentials and do not expose raw secret material in responses.
3. CI runs static + test gates without production infra credentials in workflow definition.

## 4) Findings and Risk Level

1. Critical findings: `none`.
2. High findings: `none`.
3. Medium findings:
   1. Add periodic automated IAM policy diff checks (follow-up recommended).

## 5) Decision

Least-privilege review passed for current Phase I code and CI boundaries with no unresolved critical risks.
