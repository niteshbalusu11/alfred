name_prefix = "alfred"

additional_tags = {
  owner = "alfred-dev"
}

# Image URIs from CI artifact `terraform-image-uris.auto.tfvars.json` (issue #232).
# api_image    = "<account>.dkr.ecr.us-east-2.amazonaws.com/alfred/api-server:sha-<commit>"
# worker_image = "<account>.dkr.ecr.us-east-2.amazonaws.com/alfred/worker:sha-<commit>"

# Non-secret runtime environment (hardcoded by design).
api_environment = {
  ALFRED_ENV                           = "staging"
  ENCLAVE_RUNTIME_MODE                 = "remote"
  ENCLAVE_RUNTIME_BASE_URL             = "https://enclave.alfred-dev.noderunner.wtf:8443"
  ENCLAVE_RUNTIME_PROBE_TIMEOUT_MS     = "2000"
  ENCLAVE_RPC_AUTH_MAX_SKEW_SECONDS    = "30"
  TEE_ATTESTATION_REQUIRED             = "true"
  TEE_ALLOW_INSECURE_DEV_ATTESTATION   = "false"
  TEE_EXPECTED_RUNTIME                 = "nitro"
  TEE_ALLOWED_MEASUREMENTS             = "REPLACE_WITH_DEV_MEASUREMENT"
  KMS_ALLOWED_MEASUREMENTS             = "REPLACE_WITH_DEV_MEASUREMENT"
  TEE_ATTESTATION_PUBLIC_KEY           = "REPLACE_WITH_BASE64_ED25519_PUBLIC_KEY"
  TEE_ATTESTATION_MAX_AGE_SECONDS      = "300"
  TEE_ATTESTATION_CHALLENGE_TIMEOUT_MS = "2000"
  KMS_KEY_VERSION                      = "1"
  GOOGLE_OAUTH_REDIRECT_URI            = "https://api.alfred-dev.noderunner.wtf/oauth/callback"
  CLERK_ISSUER                         = "REPLACE_WITH_CLERK_ISSUER"
  CLERK_AUDIENCE                       = "alfred-api"
  OPENROUTER_MODEL_PRIMARY             = "openai/gpt-4o-mini"
  OPENROUTER_MODEL_FALLBACK            = "anthropic/claude-3.5-haiku"
}

worker_environment = {
  ALFRED_ENV                           = "staging"
  ENCLAVE_RUNTIME_MODE                 = "remote"
  ENCLAVE_RUNTIME_BASE_URL             = "https://enclave.alfred-dev.noderunner.wtf:8443"
  ENCLAVE_RUNTIME_PROBE_TIMEOUT_MS     = "2000"
  ENCLAVE_RPC_AUTH_MAX_SKEW_SECONDS    = "30"
  TEE_ATTESTATION_REQUIRED             = "true"
  TEE_ALLOW_INSECURE_DEV_ATTESTATION   = "false"
  TEE_EXPECTED_RUNTIME                 = "nitro"
  TEE_ALLOWED_MEASUREMENTS             = "REPLACE_WITH_DEV_MEASUREMENT"
  KMS_ALLOWED_MEASUREMENTS             = "REPLACE_WITH_DEV_MEASUREMENT"
  TEE_ATTESTATION_PUBLIC_KEY           = "REPLACE_WITH_BASE64_ED25519_PUBLIC_KEY"
  TEE_ATTESTATION_MAX_AGE_SECONDS      = "300"
  TEE_ATTESTATION_CHALLENGE_TIMEOUT_MS = "2000"
  KMS_KEY_VERSION                      = "1"
  APNS_TOPIC                           = "com.prodata.alfred.dev"
  OPENROUTER_MODEL_PRIMARY             = "openai/gpt-4o-mini"
  OPENROUTER_MODEL_FALLBACK            = "anthropic/claude-3.5-haiku"
  WORKER_TICK_SECONDS                  = "30"
}

# Secret env vars from SSM (SecureString).
api_ssm_secret_arns = {
  DATABASE_URL               = "arn:aws:ssm:us-east-2:798572836804:parameter/alfred/dev/shared/DATABASE_URL"
  DATA_ENCRYPTION_KEY        = "arn:aws:ssm:us-east-2:798572836804:parameter/alfred/dev/shared/DATA_ENCRYPTION_KEY"
  KMS_KEY_ID                 = "arn:aws:ssm:us-east-2:798572836804:parameter/alfred/dev/shared/KMS_KEY_ID"
  GOOGLE_OAUTH_CLIENT_ID     = "arn:aws:ssm:us-east-2:798572836804:parameter/alfred/dev/shared/GOOGLE_OAUTH_CLIENT_ID"
  GOOGLE_OAUTH_CLIENT_SECRET = "arn:aws:ssm:us-east-2:798572836804:parameter/alfred/dev/shared/GOOGLE_OAUTH_CLIENT_SECRET"
  CLERK_SECRET_KEY           = "arn:aws:ssm:us-east-2:798572836804:parameter/alfred/dev/api/CLERK_SECRET_KEY"
  ENCLAVE_RPC_SHARED_SECRET  = "arn:aws:ssm:us-east-2:798572836804:parameter/alfred/dev/shared/ENCLAVE_RPC_SHARED_SECRET"
}

worker_ssm_secret_arns = {
  DATABASE_URL               = "arn:aws:ssm:us-east-2:798572836804:parameter/alfred/dev/shared/DATABASE_URL"
  DATA_ENCRYPTION_KEY        = "arn:aws:ssm:us-east-2:798572836804:parameter/alfred/dev/shared/DATA_ENCRYPTION_KEY"
  KMS_KEY_ID                 = "arn:aws:ssm:us-east-2:798572836804:parameter/alfred/dev/shared/KMS_KEY_ID"
  GOOGLE_OAUTH_CLIENT_ID     = "arn:aws:ssm:us-east-2:798572836804:parameter/alfred/dev/shared/GOOGLE_OAUTH_CLIENT_ID"
  GOOGLE_OAUTH_CLIENT_SECRET = "arn:aws:ssm:us-east-2:798572836804:parameter/alfred/dev/shared/GOOGLE_OAUTH_CLIENT_SECRET"
  ENCLAVE_RPC_SHARED_SECRET  = "arn:aws:ssm:us-east-2:798572836804:parameter/alfred/dev/shared/ENCLAVE_RPC_SHARED_SECRET"
  APNS_KEY_ID                = "arn:aws:ssm:us-east-2:798572836804:parameter/alfred/dev/worker/APNS_KEY_ID"
  APNS_TEAM_ID               = "arn:aws:ssm:us-east-2:798572836804:parameter/alfred/dev/worker/APNS_TEAM_ID"
  APNS_AUTH_KEY_P8_BASE64    = "arn:aws:ssm:us-east-2:798572836804:parameter/alfred/dev/worker/APNS_AUTH_KEY_P8_BASE64"
}

# Dev profile: small ECS tasks.
api_task_cpu         = 256
api_task_memory      = 512
worker_task_cpu      = 256
worker_task_memory   = 512
api_desired_count    = 1
worker_desired_count = 1

# Dev profile: small single-AZ-ish DB/cache footprint.
rds_instance_class          = "db.t4g.micro"
rds_multi_az                = false
rds_allocated_storage       = 20
rds_max_allocated_storage   = 40
rds_backup_retention_period = 0
rds_deletion_protection     = false
rds_skip_final_snapshot     = true

valkey_node_type          = "cache.t4g.micro"
valkey_num_cache_clusters = 1

# Dev profile: single enclave parent host.
enclave_instance_type = "c6i.xlarge"

# Dev profile: minimal observability cost.
log_retention_days = 7
create_alarms      = false

# HTTPS-only ingress and API TLS.
# Optional override:
# ingress_certificate_arn = "arn:aws:acm:us-east-2:123456789012:certificate/replace-dev"
# Default behavior auto-creates and DNS-validates ACM cert for api.alfred-dev.<domain>.
route53_zone_id     = "Z10154612GBUAYQKQMWC3"
route53_base_domain = "noderunner.wtf"
