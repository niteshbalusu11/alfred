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

## Dev Cost-Sensitive Defaults

`terraform/dev/terraform.tfvars` sets explicit low-cost defaults for development:

- ECS: `api` and `worker` set to `256 CPU / 512 MiB`, desired count `1`
- RDS: `db.t4g.micro`, single-AZ (`multi_az = false`), short backup retention
- Valkey: `cache.t4g.micro`, single cache node
- Enclave host: one `c6i.large` parent host
- Observability: 7-day log retention and alarms disabled by default

## Security Defaults

- Ingress supports HTTPS with ACM (`ingress_certificate_arn`), and `prod` requires a certificate ARN.
- ALB security group only opens ports enabled by ingress settings (HTTP and/or HTTPS).
- `terraform/prod/terraform.tfvars` enables safer DB lifecycle defaults:
  - `rds_deletion_protection = true`
  - `rds_skip_final_snapshot = false`
  - `rds_multi_az = true`

## Remote State Bootstrap (One-Time)

Terraform remote state is configured with:

- S3 bucket for state objects
- DynamoDB table for state locking

Create these state resources before running remote-backend init for `dev`/`prod`.

Example naming used by the provided backend examples:

- S3 bucket: `alfred-terraform-state`
- DynamoDB table: `alfred-terraform-locks`
- Region: `us-east-2`

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
