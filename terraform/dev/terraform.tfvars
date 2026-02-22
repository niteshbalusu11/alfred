name_prefix = "alfred"

additional_tags = {
  owner = "alfred-dev"
}

# Image URIs from CI artifact `terraform-image-uris.auto.tfvars.json` (issue #232).
# api_image    = "<account>.dkr.ecr.us-east-2.amazonaws.com/alfred/api-server:sha-<commit>"
# worker_image = "<account>.dkr.ecr.us-east-2.amazonaws.com/alfred/worker:sha-<commit>"

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
