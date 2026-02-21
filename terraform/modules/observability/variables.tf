variable "name_prefix" {
  description = "Naming prefix for observability resources."
  type        = string
}

variable "api_log_group_name" {
  description = "CloudWatch log group for API service."
  type        = string
}

variable "worker_log_group_name" {
  description = "CloudWatch log group for worker service."
  type        = string
}

variable "enclave_log_group_name" {
  description = "CloudWatch log group for enclave parent host."
  type        = string
}

variable "log_retention_days" {
  description = "CloudWatch log retention period."
  type        = number
  default     = 14
}

variable "create_alarms" {
  description = "Whether to create baseline CloudWatch alarms."
  type        = bool
  default     = true
}

variable "alarm_actions" {
  description = "SNS topic ARNs or other alarm action ARNs."
  type        = list(string)
  default     = []
}

variable "alb_arn_suffix" {
  description = "ALB ARN suffix for metrics dimensions."
  type        = string
  default     = null
}

variable "target_group_arn_suffix" {
  description = "Target group ARN suffix for metrics dimensions."
  type        = string
  default     = null
}

variable "ecs_cluster_name" {
  description = "ECS cluster name for service alarms."
  type        = string
  default     = null
}

variable "api_service_name" {
  description = "ECS API service name for alarms."
  type        = string
  default     = null
}

variable "rds_identifier" {
  description = "RDS instance identifier for alarms."
  type        = string
  default     = null
}

variable "valkey_replication_group_id" {
  description = "Valkey replication group ID for alarms."
  type        = string
  default     = null
}

variable "api_cpu_alarm_threshold" {
  description = "CPU threshold for API ECS service alarm."
  type        = number
  default     = 80
}

variable "rds_cpu_alarm_threshold" {
  description = "CPU threshold for RDS alarm."
  type        = number
  default     = 80
}

variable "valkey_cpu_alarm_threshold" {
  description = "CPU threshold for Valkey alarm."
  type        = number
  default     = 75
}
