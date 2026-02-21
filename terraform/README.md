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

## Dev Cost-Sensitive Defaults

`terraform/dev/terraform.tfvars` sets explicit low-cost defaults for development:

- ECS: `api` and `worker` set to `256 CPU / 512 MiB`, desired count `1`
- RDS: `db.t4g.micro`, single-AZ (`multi_az = false`), short backup retention
- Valkey: `cache.t4g.micro`, single cache node
- Enclave host: one `c6i.large` parent host
- Observability: 7-day log retention and alarms disabled by default

## Prod Reliability Defaults

`terraform/prod/terraform.tfvars` sets explicit production defaults while reusing the same shared module graph:

- ECS: `api` and `worker` set to `1024 CPU / 2048 MiB`, desired count `2`
- RDS: `db.t4g.medium`, Multi-AZ enabled, larger storage, 14-day backups, deletion protection enabled
- Valkey: `cache.t4g.small`, two cache nodes
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
| `rds_multi_az` | `false` | `true` |
| `rds_allocated_storage` | `20` | `100` |
| `rds_max_allocated_storage` | `40` | `300` |
| `rds_backup_retention_period` | `1` | `14` |
| `rds_deletion_protection` | `false` | `true` |
| `rds_skip_final_snapshot` | `true` | `false` |
| `valkey_node_type` | `cache.t4g.micro` | `cache.t4g.small` |
| `valkey_num_cache_clusters` | `1` | `2` |
| `enclave_instance_type` | `c6i.large` | `c7i.xlarge` |
| `alb_deletion_protection` | module default (`false`) | `true` |
| `log_retention_days` | `7` | `30` |
| `create_alarms` | `false` | `true` |

## Security Defaults

- Public ingress is HTTPS-only on `443` with ACM (`ingress_certificate_arn` required in both envs).
- HTTP listener is not created.
- ALB security group opens `443` only.
- ALB target group to API uses HTTPS, and API traffic is expected on TLS port `8443`.
- Internal runtime API access rules (API/worker/enclave) are on TLS port `8443` only.
- Route53 alias records are created for API when `route53_zone_id` and `route53_base_domain` are set:
  - `api.alfred-dev.<domain>`
  - `api.alfred-prod.<domain>`
- Worker/enclave remain private services; Terraform outputs suggested names for future private DNS.
- `terraform/prod/terraform.tfvars` uses production-oriented capacity and lifecycle defaults (see matrix above).

## Remote State Bootstrap (One-Time)

Terraform remote state is configured with:

- S3 bucket for state objects
- DynamoDB table for state locking

Create these state resources before running remote-backend init for `dev`/`prod`.

Example naming used by the provided backend examples:

- S3 bucket: `alfred-terraform-state`
- DynamoDB table: `alfred-terraform-locks`
- Region: `us-east-2`
- Hosted zone in current env tfvars: `Z10154612GBUAYQKQMWC3` (`noderunner.wtf`)

## Environment Setup

1. Copy backend config:
   - `cp terraform/dev/backend.hcl.example terraform/dev/backend.hcl`
   - `cp terraform/prod/backend.hcl.example terraform/prod/backend.hcl`
2. Update bucket/table names if your account uses different names.
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

If the shared S3/DynamoDB state resources are not created yet, validate configuration only:

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
