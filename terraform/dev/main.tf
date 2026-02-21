module "environment" {
  source = "../modules"

  environment = "dev"
  aws_region  = var.aws_region
  name_prefix = var.name_prefix
  tags        = var.additional_tags

  ingress_certificate_arn         = var.ingress_certificate_arn
  ingress_auto_create_certificate = var.ingress_auto_create_certificate
  route53_zone_id                 = var.route53_zone_id
  route53_base_domain             = var.route53_base_domain
  api_image                       = var.api_image
  worker_image                    = var.worker_image

  api_task_cpu         = var.api_task_cpu
  api_task_memory      = var.api_task_memory
  worker_task_cpu      = var.worker_task_cpu
  worker_task_memory   = var.worker_task_memory
  api_desired_count    = var.api_desired_count
  worker_desired_count = var.worker_desired_count

  rds_instance_class          = var.rds_instance_class
  rds_engine_version          = var.rds_engine_version
  rds_multi_az                = var.rds_multi_az
  rds_allocated_storage       = var.rds_allocated_storage
  rds_max_allocated_storage   = var.rds_max_allocated_storage
  rds_backup_retention_period = var.rds_backup_retention_period
  rds_deletion_protection     = var.rds_deletion_protection
  rds_skip_final_snapshot     = var.rds_skip_final_snapshot

  valkey_node_type          = var.valkey_node_type
  valkey_num_cache_clusters = var.valkey_num_cache_clusters
  valkey_engine_version     = var.valkey_engine_version

  enclave_instance_type = var.enclave_instance_type

  log_retention_days = var.log_retention_days
  create_alarms      = var.create_alarms
}
