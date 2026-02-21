variable "aws_region" {
  description = "AWS region for the dev environment."
  type        = string
  default     = "us-east-2"
}

variable "name_prefix" {
  description = "Resource naming prefix."
  type        = string
}

variable "additional_tags" {
  description = "Additional tags applied on top of default tags."
  type        = map(string)
  default     = {}
}

variable "ingress_certificate_arn" {
  description = "Optional ACM certificate ARN for HTTPS ingress. Leave unset to auto-manage certificate via Route53."
  type        = string
  default     = null
}

variable "ingress_auto_create_certificate" {
  description = "Whether to auto-create and DNS-validate an ACM certificate when ingress_certificate_arn is unset."
  type        = bool
  default     = true
}

variable "route53_zone_id" {
  description = "Route53 hosted zone ID for API DNS record."
  type        = string
}

variable "route53_base_domain" {
  description = "Base domain for API DNS record."
  type        = string
}

variable "api_image" {
  description = "Container image URI for API service."
  type        = string
  default     = "public.ecr.aws/docker/library/nginx:latest"
}

variable "worker_image" {
  description = "Container image URI for worker service."
  type        = string
  default     = "public.ecr.aws/docker/library/nginx:latest"
}

variable "api_task_cpu" {
  description = "Fargate CPU units for API service."
  type        = number
  default     = 512
}

variable "api_task_memory" {
  description = "Fargate memory (MiB) for API service."
  type        = number
  default     = 1024
}

variable "worker_task_cpu" {
  description = "Fargate CPU units for worker service."
  type        = number
  default     = 512
}

variable "worker_task_memory" {
  description = "Fargate memory (MiB) for worker service."
  type        = number
  default     = 1024
}

variable "api_desired_count" {
  description = "Desired API service task count."
  type        = number
  default     = 1
}

variable "worker_desired_count" {
  description = "Desired worker service task count."
  type        = number
  default     = 1
}

variable "rds_instance_class" {
  description = "RDS instance class."
  type        = string
  default     = "db.t4g.micro"
}

variable "rds_multi_az" {
  description = "Enable Multi-AZ for RDS."
  type        = bool
  default     = true
}

variable "rds_allocated_storage" {
  description = "RDS allocated storage in GB."
  type        = number
  default     = 20
}

variable "rds_max_allocated_storage" {
  description = "RDS max auto-scaled storage in GB."
  type        = number
  default     = 100
}

variable "rds_backup_retention_period" {
  description = "RDS backup retention period in days."
  type        = number
  default     = 7
}

variable "rds_deletion_protection" {
  description = "Enable RDS deletion protection."
  type        = bool
  default     = true
}

variable "rds_skip_final_snapshot" {
  description = "Skip final snapshot on RDS deletion."
  type        = bool
  default     = false
}

variable "valkey_node_type" {
  description = "Valkey node type."
  type        = string
  default     = "cache.t4g.micro"
}

variable "valkey_num_cache_clusters" {
  description = "Number of Valkey cache clusters."
  type        = number
  default     = 1
}

variable "enclave_instance_type" {
  description = "Enclave parent host instance type."
  type        = string
  default     = "c7i.large"
}

variable "log_retention_days" {
  description = "CloudWatch log retention in days."
  type        = number
  default     = 14
}

variable "create_alarms" {
  description = "Whether to create baseline alarms."
  type        = bool
  default     = true
}
