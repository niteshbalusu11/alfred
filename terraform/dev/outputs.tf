output "environment" {
  description = "Environment identifier."
  value       = module.environment.environment
}

output "aws_region" {
  description = "AWS region used by this environment."
  value       = module.environment.aws_region
}

output "stack_name" {
  description = "Canonical stack name prefix."
  value       = module.environment.stack_name
}

output "tags" {
  description = "Resolved default and custom tags."
  value       = module.environment.tags
}

output "api_endpoint" {
  description = "Public API endpoint DNS."
  value       = module.environment.api_endpoint
}

output "api_fqdn" {
  description = "Public API FQDN."
  value       = module.environment.api_fqdn
}

output "api_target_group_arn" {
  description = "API target group ARN."
  value       = module.environment.api_target_group_arn
}

output "ecs_cluster_name" {
  description = "ECS cluster name."
  value       = module.environment.ecs_cluster_name
}

output "api_service_name" {
  description = "API ECS service name."
  value       = module.environment.api_service_name
}

output "worker_service_name" {
  description = "Worker ECS service name."
  value       = module.environment.worker_service_name
}

output "rds_endpoint" {
  description = "RDS endpoint hostname."
  value       = module.environment.rds_endpoint
}

output "rds_port" {
  description = "RDS endpoint port."
  value       = module.environment.rds_port
}

output "valkey_primary_endpoint" {
  description = "Valkey primary endpoint hostname."
  value       = module.environment.valkey_primary_endpoint
}

output "valkey_port" {
  description = "Valkey endpoint port."
  value       = module.environment.valkey_port
}

output "vpc_id" {
  description = "VPC identifier."
  value       = module.environment.vpc_id
}

output "private_subnet_ids" {
  description = "Private subnet identifiers."
  value       = module.environment.private_subnet_ids
}

output "enclave_host_instance_id" {
  description = "Enclave parent host instance ID."
  value       = module.environment.enclave_host_instance_id
}

output "enclave_host_private_ip" {
  description = "Enclave parent host private IP."
  value       = module.environment.enclave_host_private_ip
}

output "cloudwatch_log_groups" {
  description = "CloudWatch log group names."
  value       = module.environment.cloudwatch_log_groups
}

output "alarm_names" {
  description = "Created alarm names."
  value       = module.environment.alarm_names
}

output "worker_suggested_fqdn" {
  description = "Suggested worker DNS name for future private DNS routing."
  value       = module.environment.worker_suggested_fqdn
}

output "enclave_suggested_fqdn" {
  description = "Suggested enclave DNS name for future private DNS routing."
  value       = module.environment.enclave_suggested_fqdn
}
