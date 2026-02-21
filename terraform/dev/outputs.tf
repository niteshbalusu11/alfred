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
