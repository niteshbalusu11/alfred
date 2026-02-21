output "alb_security_group_id" {
  description = "ALB security group ID."
  value       = aws_security_group.alb.id
}

output "api_security_group_id" {
  description = "API ECS service security group ID."
  value       = aws_security_group.api.id
}

output "worker_security_group_id" {
  description = "Worker ECS service security group ID."
  value       = aws_security_group.worker.id
}

output "enclave_security_group_id" {
  description = "Enclave parent host security group ID."
  value       = aws_security_group.enclave.id
}

output "db_security_group_id" {
  description = "RDS security group ID."
  value       = aws_security_group.db.id
}

output "cache_security_group_id" {
  description = "Valkey security group ID."
  value       = aws_security_group.cache.id
}
