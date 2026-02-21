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
