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

output "ecs_cluster_name" {
  description = "ECS cluster name."
  value       = module.environment.ecs_cluster_name
}

output "rds_endpoint" {
  description = "RDS endpoint hostname."
  value       = module.environment.rds_endpoint
}

output "valkey_primary_endpoint" {
  description = "Valkey primary endpoint hostname."
  value       = module.environment.valkey_primary_endpoint
}

output "worker_suggested_fqdn" {
  description = "Suggested worker DNS name for future private DNS routing."
  value       = module.environment.worker_suggested_fqdn
}

output "enclave_suggested_fqdn" {
  description = "Suggested enclave DNS name for future private DNS routing."
  value       = module.environment.enclave_suggested_fqdn
}
