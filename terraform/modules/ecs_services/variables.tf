variable "name_prefix" {
  description = "Naming prefix for ECS resources."
  type        = string
}

variable "aws_region" {
  description = "AWS region for logs and task definitions."
  type        = string
}

variable "private_subnet_ids" {
  description = "Private subnet IDs for ECS services."
  type        = list(string)
}

variable "api_security_group_id" {
  description = "Security group ID for API ECS service."
  type        = string
}

variable "worker_security_group_id" {
  description = "Security group ID for worker ECS service."
  type        = string
}

variable "ecs_task_execution_role_arn" {
  description = "Execution role ARN for ECS tasks."
  type        = string
}

variable "api_task_role_arn" {
  description = "Task role ARN for API service."
  type        = string
}

variable "worker_task_role_arn" {
  description = "Task role ARN for worker service."
  type        = string
}

variable "cluster_name" {
  description = "ECS cluster name."
  type        = string
}

variable "api_service_name" {
  description = "API service name."
  type        = string
}

variable "worker_service_name" {
  description = "Worker service name."
  type        = string
}

variable "api_image" {
  description = "Container image URI for API service."
  type        = string
}

variable "worker_image" {
  description = "Container image URI for worker service."
  type        = string
}

variable "api_container_port" {
  description = "API TLS container port."
  type        = number
  default     = 8443
}

variable "api_task_cpu" {
  description = "API Fargate task CPU units."
  type        = number
  default     = 512
}

variable "api_task_memory" {
  description = "API Fargate task memory in MiB."
  type        = number
  default     = 1024
}

variable "worker_task_cpu" {
  description = "Worker Fargate task CPU units."
  type        = number
  default     = 512
}

variable "worker_task_memory" {
  description = "Worker Fargate task memory in MiB."
  type        = number
  default     = 1024
}

variable "api_desired_count" {
  description = "Desired task count for API service."
  type        = number
  default     = 1
}

variable "worker_desired_count" {
  description = "Desired task count for worker service."
  type        = number
  default     = 1
}

variable "api_target_group_arn" {
  description = "ALB target group ARN for API service."
  type        = string
}

variable "api_log_group_name" {
  description = "CloudWatch log group name for API service."
  type        = string
}

variable "worker_log_group_name" {
  description = "CloudWatch log group name for worker service."
  type        = string
}

variable "api_environment" {
  description = "Non-secret environment variables for API container."
  type        = map(string)
  default     = {}
}

variable "worker_environment" {
  description = "Non-secret environment variables for worker container."
  type        = map(string)
  default     = {}
}

variable "api_secrets" {
  description = "Secrets list for API ECS container definitions."
  type = list(object({
    name      = string
    valueFrom = string
  }))
  default = []
}

variable "worker_secrets" {
  description = "Secrets list for worker ECS container definitions."
  type = list(object({
    name      = string
    valueFrom = string
  }))
  default = []
}
