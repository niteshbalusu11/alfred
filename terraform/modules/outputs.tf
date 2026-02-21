output "environment" {
  description = "Environment identifier."
  value       = var.environment
}

output "aws_region" {
  description = "AWS region configured for this environment."
  value       = var.aws_region
}

output "stack_name" {
  description = "Canonical stack name prefix for this environment."
  value       = local.stack_name
}

output "tags" {
  description = "Resolved baseline tags."
  value       = local.effective_tags
}

output "vpc_id" {
  description = "VPC identifier."
  value       = module.network.vpc_id
}

output "public_subnet_ids" {
  description = "Public subnet identifiers."
  value       = module.network.public_subnet_ids
}

output "private_subnet_ids" {
  description = "Private subnet identifiers."
  value       = module.network.private_subnet_ids
}

output "api_endpoint" {
  description = "Public API endpoint DNS via ALB."
  value       = module.ingress.alb_dns_name
}

output "api_target_group_arn" {
  description = "API target group ARN."
  value       = module.ingress.target_group_arn
}

output "ecs_cluster_name" {
  description = "ECS cluster name."
  value       = module.ecs_services.cluster_name
}

output "api_service_name" {
  description = "API ECS service name."
  value       = module.ecs_services.api_service_name
}

output "worker_service_name" {
  description = "Worker ECS service name."
  value       = module.ecs_services.worker_service_name
}

output "rds_endpoint" {
  description = "RDS endpoint hostname."
  value       = module.rds_postgres.endpoint
}

output "rds_port" {
  description = "RDS endpoint port."
  value       = module.rds_postgres.port
}

output "valkey_primary_endpoint" {
  description = "Valkey primary endpoint hostname."
  value       = module.valkey.primary_endpoint
}

output "valkey_port" {
  description = "Valkey endpoint port."
  value       = module.valkey.port
}

output "enclave_host_instance_id" {
  description = "Enclave parent host instance ID."
  value       = module.enclave_host.instance_id
}

output "enclave_host_private_ip" {
  description = "Enclave parent host private IP."
  value       = module.enclave_host.private_ip
}

output "cloudwatch_log_groups" {
  description = "CloudWatch log groups for runtime services."
  value = {
    api     = module.observability.api_log_group_name
    worker  = module.observability.worker_log_group_name
    enclave = module.observability.enclave_log_group_name
  }
}

output "alarm_names" {
  description = "Created baseline CloudWatch alarm names."
  value       = module.observability.alarm_names
}
