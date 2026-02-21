locals {
  stack_name = "${var.name_prefix}-${var.environment}"

  default_tags = {
    project     = "alfred"
    environment = var.environment
    managed_by  = "terraform"
  }

  effective_tags = merge(local.default_tags, var.tags)

  ecs_cluster_name    = "${local.stack_name}-cluster"
  api_service_name    = "${local.stack_name}-api"
  worker_service_name = "${local.stack_name}-worker"

  api_log_group_name     = "/alfred/${var.environment}/api-server"
  worker_log_group_name  = "/alfred/${var.environment}/worker"
  enclave_log_group_name = "/alfred/${var.environment}/enclave-host"
}

module "network" {
  source = "./network"

  name_prefix          = local.stack_name
  vpc_cidr             = var.vpc_cidr
  availability_zones   = var.availability_zones
  public_subnet_cidrs  = var.public_subnet_cidrs
  private_subnet_cidrs = var.private_subnet_cidrs
  enable_nat_gateway   = var.enable_nat_gateway
}

module "security_groups" {
  source = "./security_groups"

  name_prefix = local.stack_name
  vpc_id      = module.network.vpc_id
  api_port    = var.api_port
  db_port     = var.db_port
  cache_port  = var.cache_port
}

module "secrets_wiring" {
  source = "./secrets_wiring"

  api_ssm_secret_arns          = var.api_ssm_secret_arns
  api_secrets_manager_arns     = var.api_secrets_manager_arns
  worker_ssm_secret_arns       = var.worker_ssm_secret_arns
  worker_secrets_manager_arns  = var.worker_secrets_manager_arns
  enclave_ssm_secret_arns      = var.enclave_ssm_secret_arns
  enclave_secrets_manager_arns = var.enclave_secrets_manager_arns
}

module "iam_runtime" {
  source = "./iam_runtime"

  name_prefix          = local.stack_name
  ssm_parameter_arns   = module.secrets_wiring.all_ssm_secret_arns
  secrets_manager_arns = module.secrets_wiring.all_secrets_manager_arns
  kms_key_arns         = var.kms_key_arns
}

module "ingress" {
  source = "./ingress"

  name_prefix         = local.stack_name
  vpc_id              = module.network.vpc_id
  public_subnet_ids   = module.network.public_subnet_ids
  security_group_id   = module.security_groups.alb_security_group_id
  target_port         = var.api_port
  health_check_path   = var.ingress_health_check_path
  deletion_protection = var.alb_deletion_protection
  certificate_arn     = var.ingress_certificate_arn
  ssl_policy          = var.ingress_ssl_policy
}

module "rds_postgres" {
  source = "./rds_postgres"

  name_prefix             = local.stack_name
  subnet_ids              = module.network.private_subnet_ids
  security_group_id       = module.security_groups.db_security_group_id
  db_name                 = var.rds_db_name
  master_username         = var.rds_master_username
  instance_class          = var.rds_instance_class
  engine_version          = var.rds_engine_version
  allocated_storage       = var.rds_allocated_storage
  max_allocated_storage   = var.rds_max_allocated_storage
  multi_az                = var.rds_multi_az
  backup_retention_period = var.rds_backup_retention_period
  deletion_protection     = var.rds_deletion_protection
  skip_final_snapshot     = var.rds_skip_final_snapshot
}

module "valkey" {
  source = "./valkey"

  name_prefix          = local.stack_name
  subnet_ids           = module.network.private_subnet_ids
  security_group_id    = module.security_groups.cache_security_group_id
  node_type            = var.valkey_node_type
  num_cache_clusters   = var.valkey_num_cache_clusters
  port                 = var.cache_port
  parameter_group_name = var.valkey_parameter_group_name
}

module "ecs_services" {
  source = "./ecs_services"

  name_prefix                 = local.stack_name
  aws_region                  = var.aws_region
  private_subnet_ids          = module.network.private_subnet_ids
  api_security_group_id       = module.security_groups.api_security_group_id
  worker_security_group_id    = module.security_groups.worker_security_group_id
  ecs_task_execution_role_arn = module.iam_runtime.ecs_task_execution_role_arn
  api_task_role_arn           = module.iam_runtime.api_task_role_arn
  worker_task_role_arn        = module.iam_runtime.worker_task_role_arn
  cluster_name                = local.ecs_cluster_name
  api_service_name            = local.api_service_name
  worker_service_name         = local.worker_service_name
  api_image                   = var.api_image
  worker_image                = var.worker_image
  api_container_port          = var.api_port
  api_task_cpu                = var.api_task_cpu
  api_task_memory             = var.api_task_memory
  worker_task_cpu             = var.worker_task_cpu
  worker_task_memory          = var.worker_task_memory
  api_desired_count           = var.api_desired_count
  worker_desired_count        = var.worker_desired_count
  api_target_group_arn        = module.ingress.target_group_arn
  api_log_group_name          = module.observability.api_log_group_name
  worker_log_group_name       = module.observability.worker_log_group_name
  api_environment             = var.api_environment
  worker_environment          = var.worker_environment
  api_secrets                 = module.secrets_wiring.api_ecs_secrets
  worker_secrets              = module.secrets_wiring.worker_ecs_secrets

  depends_on = [
    module.ingress,
    module.observability
  ]
}

module "enclave_host" {
  source = "./enclave_host"

  name_prefix               = local.stack_name
  subnet_id                 = module.network.private_subnet_ids[0]
  security_group_id         = module.security_groups.enclave_security_group_id
  ami_id                    = var.enclave_ami_id
  instance_type             = var.enclave_instance_type
  iam_instance_profile_name = module.iam_runtime.enclave_host_instance_profile_name
  key_name                  = var.enclave_key_name
  root_volume_size          = var.enclave_root_volume_size
  user_data                 = var.enclave_user_data
}

module "observability" {
  source = "./observability"

  name_prefix                 = local.stack_name
  api_log_group_name          = local.api_log_group_name
  worker_log_group_name       = local.worker_log_group_name
  enclave_log_group_name      = local.enclave_log_group_name
  log_retention_days          = var.log_retention_days
  create_alarms               = var.create_alarms
  alarm_actions               = var.alarm_actions
  alb_arn_suffix              = module.ingress.alb_arn_suffix
  target_group_arn_suffix     = module.ingress.target_group_arn_suffix
  ecs_cluster_name            = local.ecs_cluster_name
  api_service_name            = local.api_service_name
  rds_identifier              = module.rds_postgres.identifier
  valkey_replication_group_id = module.valkey.replication_group_id
}
