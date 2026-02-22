# Alfred Terraform Bootstrap

This directory contains Terraform scaffolding for the Alfred AWS deployment baseline in `us-east-2`.

## Layout

- `modules/`: reusable Terraform modules shared by environments.
- `dev/`: development composition wrapper.
- `prod/`: production composition wrapper.

Environment roots stay thin: they configure backend/provider defaults and call shared modules.

Current runtime module graph under `modules/`:

- `network`
- `security_groups`
- `ingress`
- `ecs_services`
- `enclave_host`
- `rds_postgres`
- `valkey`
- `iam_runtime`
- `secrets_wiring`
- `observability`

Image inputs consumed by environment wrappers:

- `api_image`
- `worker_image`

Runtime env inputs consumed by environment wrappers:

- `api_environment` (non-secret env vars)
- `worker_environment` (non-secret env vars)
- `api_ssm_secret_arns` (API secret env var name -> SSM parameter ARN)
- `worker_ssm_secret_arns` (worker secret env var name -> SSM parameter ARN)

## Dev Cost-Sensitive Defaults

`terraform/dev/terraform.tfvars` sets explicit low-cost defaults for development:

- ECS: `api` and `worker` set to `256 CPU / 512 MiB`, desired count `1`
- RDS: `db.t4g.micro`, `postgres 18`, single-AZ (`multi_az = false`), automated backups disabled
- Valkey: `cache.t4g.micro`, `valkey 8.2`, single cache node
- Enclave host: one `c6i.xlarge` parent host
- Observability: 7-day log retention and alarms disabled by default

## Prod Reliability Defaults

`terraform/prod/terraform.tfvars` sets explicit production defaults while reusing the same shared module graph:

- ECS: `api` and `worker` set to `1024 CPU / 2048 MiB`, desired count `2`
- RDS: `db.t4g.medium`, `postgres 18`, Multi-AZ enabled, larger storage, 14-day backups, deletion protection enabled
- Valkey: `cache.t4g.small`, `valkey 8.2`, two cache nodes
- Enclave host: one `c7i.xlarge` parent host
- Observability: 30-day log retention and alarms enabled
- ALB deletion protection enabled

## Dev vs Prod Variable Matrix

All environment differences are variable-driven through `terraform/dev` and `terraform/prod`; no module logic is duplicated.

| Variable | Dev | Prod |
| --- | --- | --- |
| `api_task_cpu` | `256` | `1024` |
| `api_task_memory` | `512` | `2048` |
| `worker_task_cpu` | `256` | `1024` |
| `worker_task_memory` | `512` | `2048` |
| `api_desired_count` | `1` | `2` |
| `worker_desired_count` | `1` | `2` |
| `rds_instance_class` | `db.t4g.micro` | `db.t4g.medium` |
| `rds_engine_version` | `18` | `18` |
| `rds_multi_az` | `false` | `true` |
| `rds_allocated_storage` | `20` | `100` |
| `rds_max_allocated_storage` | `40` | `300` |
| `rds_backup_retention_period` | `0` | `14` |
| `rds_deletion_protection` | `false` | `true` |
| `rds_skip_final_snapshot` | `true` | `false` |
| `valkey_node_type` | `cache.t4g.micro` | `cache.t4g.small` |
| `valkey_num_cache_clusters` | `1` | `2` |
| `valkey_engine_version` | `8.2` | `8.2` |
| `enclave_instance_type` | `c6i.xlarge` | `c7i.xlarge` |
| `alb_deletion_protection` | module default (`false`) | `true` |
| `log_retention_days` | `7` | `30` |
| `create_alarms` | `false` | `true` |

## Security Defaults

- Public ingress is HTTPS-only on `443` with ACM.
- ACM certificate is auto-created and DNS-validated when `route53_zone_id` + `route53_base_domain` are set.
- `ingress_certificate_arn` remains available as an optional override if you want to reuse an existing certificate.
- HTTP listener is not created.
- ALB security group opens `443` only.
- ALB target group to API uses HTTPS, and API traffic is expected on TLS port `8443`.
- Internal runtime API access rules (API/worker/enclave) are on TLS port `8443` only.
- Route53 alias records are created for API when `route53_zone_id` and `route53_base_domain` are set:
  - `api.alfred-dev.<domain>`
  - `api.alfred-prod.<domain>`
- Worker/enclave remain private services; Terraform outputs suggested names for future private DNS.
- `terraform/prod/terraform.tfvars` uses production-oriented capacity and lifecycle defaults (see matrix above).

## Runtime Secret Handling

- App/runtime secrets are not auto-created by Terraform.
- Store secrets as SSM `SecureString` parameters.
- Inject secrets into ECS by setting:
  - `api_ssm_secret_arns`
  - `worker_ssm_secret_arns`
- Keep non-secret runtime configuration in:
  - `api_environment`
  - `worker_environment`
- Current tfvars convention uses these SSM parameter paths:
  - `alfred/<env>/shared/DATABASE_URL`
  - `alfred/<env>/shared/DATA_ENCRYPTION_KEY`
  - `alfred/<env>/shared/GOOGLE_OAUTH_CLIENT_SECRET`
  - `alfred/<env>/shared/ENCLAVE_RPC_SHARED_SECRET`
  - `alfred/<env>/api/CLERK_SECRET_KEY`
  - `alfred/<env>/worker/APNS_AUTH_KEY_P8_BASE64`

## Remote State Bootstrap (One-Time)

Terraform remote state is configured with:

- S3 bucket for state objects
- S3 lockfile-based state locking (`use_lockfile = true`)

Create these state resources before running remote-backend init for `dev`/`prod`.

Example naming used by the provided backend examples:

- S3 bucket: `alfred-terraform-state`
- Region: `us-east-2`
- Hosted zone in current env tfvars: `Z10154612GBUAYQKQMWC3` (`noderunner.wtf`)

## Environment Setup

1. Copy backend config:
   - `cp terraform/dev/backend.hcl.example terraform/dev/backend.hcl`
   - `cp terraform/prod/backend.hcl.example terraform/prod/backend.hcl`
2. Update bucket/region names if your account uses different names.
3. Initialize each environment:

```bash
cd terraform/dev
terraform init -backend-config=backend.hcl
terraform plan -var-file=terraform.tfvars
terraform apply -var-file=terraform.tfvars
```

```bash
cd terraform/prod
terraform init -backend-config=backend.hcl
terraform plan -var-file=terraform.tfvars
terraform apply -var-file=terraform.tfvars
```

## Local Validation Without Remote State

If the shared state resources are not created yet, validate configuration only:

```bash
cd terraform/dev
terraform init -backend=false
terraform validate
```

```bash
cd terraform/prod
terraform init -backend=false
terraform validate
```

## Image Pipeline Handoff (Issue #232)

Runtime images are built/published by `.github/workflows/ecr-images.yml` and exported as a workflow artifact named `terraform-image-uris` containing:

```json
{
  "api_image": "<account>.dkr.ecr.us-east-2.amazonaws.com/alfred/api-server:sha-<commit>",
  "worker_image": "<account>.dkr.ecr.us-east-2.amazonaws.com/alfred/worker:sha-<commit>",
  "enclave_runtime_image": "<account>.dkr.ecr.us-east-2.amazonaws.com/alfred/enclave-runtime:sha-<commit>"
}
```

Use `api_image` and `worker_image` values in `terraform/dev/terraform.tfvars` and `terraform/prod/terraform.tfvars`, or pass the artifact file directly:

```bash
cd terraform/prod
terraform plan \
  -var-file=terraform.tfvars \
  -var-file=terraform-image-uris.auto.tfvars.json
```

## Terraform CI/CD (Dev)

Workflow: `.github/workflows/terraform-dev.yml`

1. PRs to `master` (Terraform/workflow path changes) run:
   1. `terraform fmt -check -recursive`
   2. `terraform validate`
   3. `terraform plan` against the shared dev state backend
2. Manual dev deploy test is supported via `workflow_dispatch`:
   1. set `apply=true`
   2. optional input overrides for image URIs, certificate ARN override, and Route53 values

Required GitHub secret:

1. `AWS_TERRAFORM_DEV_ROLE_ARN` (OIDC-assumable role for Terraform plan/apply in dev)

Optional GitHub repository variables:

1. `TF_STATE_BUCKET` (defaults to `alfred-terraform-state`)
2. `TF_STATE_REGION` (defaults to `AWS_REGION` in workflow)
3. `TF_STATE_KEY_DEV` (defaults to `dev/terraform.tfstate`)
4. `DEV_API_IMAGE`
5. `DEV_WORKER_IMAGE`
6. `DEV_ROUTE53_ZONE_ID`
7. `DEV_ROUTE53_BASE_DOMAIN`
