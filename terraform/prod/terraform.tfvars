name_prefix = "alfred"

additional_tags = {
  owner = "alfred-prod"
}

# Prod profile: larger ECS tasks and multi-task capacity.
api_task_cpu         = 1024
api_task_memory      = 2048
worker_task_cpu      = 1024
worker_task_memory   = 2048
api_desired_count    = 2
worker_desired_count = 2

# Prod profile: resilient data + cache sizing.
rds_instance_class          = "db.t4g.medium"
rds_allocated_storage       = 100
rds_max_allocated_storage   = 300
rds_backup_retention_period = 14
rds_deletion_protection     = true
rds_skip_final_snapshot     = false
rds_multi_az                = true

# Prod profile: larger cache and enclave host.
valkey_node_type          = "cache.t4g.small"
valkey_num_cache_clusters = 2
enclave_instance_type     = "c7i.xlarge"

# Prod profile: safety + observability defaults.
alb_deletion_protection = true
log_retention_days      = 30
create_alarms           = true

# Prod should use TLS at ingress. Replace with real ACM cert ARN before apply.
ingress_certificate_arn = "arn:aws:acm:us-east-2:123456789012:certificate/replace-me"
route53_zone_id         = "Z10154612GBUAYQKQMWC3"
route53_base_domain     = "noderunner.wtf"
