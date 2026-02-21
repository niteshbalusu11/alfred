output "ecs_task_execution_role_arn" {
  description = "ECS task execution role ARN."
  value       = aws_iam_role.ecs_task_execution.arn
}

output "api_task_role_arn" {
  description = "API task role ARN."
  value       = aws_iam_role.api_task.arn
}

output "worker_task_role_arn" {
  description = "Worker task role ARN."
  value       = aws_iam_role.worker_task.arn
}

output "enclave_host_instance_profile_name" {
  description = "EC2 instance profile name for enclave parent host."
  value       = aws_iam_instance_profile.enclave_host.name
}

output "enclave_host_role_arn" {
  description = "Enclave parent host IAM role ARN."
  value       = aws_iam_role.enclave_host.arn
}
