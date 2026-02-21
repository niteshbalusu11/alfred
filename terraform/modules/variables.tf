variable "environment" {
  description = "Environment name (for example: dev, prod)."
  type        = string
}

variable "aws_region" {
  description = "AWS region for this environment."
  type        = string
}

variable "name_prefix" {
  description = "Resource naming prefix."
  type        = string
}

variable "tags" {
  description = "Additional tags to merge into module defaults."
  type        = map(string)
  default     = {}
}

variable "vpc_cidr" {
  description = "VPC CIDR block."
  type        = string
  default     = "10.40.0.0/16"
}

variable "availability_zones" {
  description = "Availability zones used for public/private subnets."
  type        = list(string)
  default     = ["us-east-2a", "us-east-2b"]
}

variable "public_subnet_cidrs" {
  description = "Public subnet CIDR blocks."
  type        = list(string)
  default     = ["10.40.0.0/20", "10.40.16.0/20"]
}

variable "private_subnet_cidrs" {
  description = "Private subnet CIDR blocks."
  type        = list(string)
  default     = ["10.40.128.0/20", "10.40.144.0/20"]
}

variable "enable_nat_gateway" {
  description = "Whether to create a NAT gateway for private subnet egress."
  type        = bool
  default     = true
}

variable "api_port" {
  description = "API TLS port exposed behind ALB."
  type        = number
  default     = 8443
}

variable "db_port" {
  description = "PostgreSQL port."
  type        = number
  default     = 5432
}

variable "cache_port" {
  description = "Valkey port."
  type        = number
  default     = 6379
}

variable "api_image" {
  description = "Container image URI for api-server ECS task."
  type        = string
  default     = "public.ecr.aws/docker/library/nginx:latest"
}

variable "worker_image" {
  description = "Container image URI for worker ECS task."
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
  description = "Desired ECS task count for API service."
  type        = number
  default     = 1
}

variable "worker_desired_count" {
  description = "Desired ECS task count for worker service."
  type        = number
  default     = 1
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

variable "api_ssm_secret_arns" {
  description = "Map of API env var name to SSM parameter ARN."
  type        = map(string)
  default     = {}
}

variable "api_secrets_manager_arns" {
  description = "Map of API env var name to Secrets Manager ARN."
  type        = map(string)
  default     = {}
}

variable "worker_ssm_secret_arns" {
  description = "Map of worker env var name to SSM parameter ARN."
  type        = map(string)
  default     = {}
}

variable "worker_secrets_manager_arns" {
  description = "Map of worker env var name to Secrets Manager ARN."
  type        = map(string)
  default     = {}
}

variable "enclave_ssm_secret_arns" {
  description = "Map of enclave env var name to SSM parameter ARN."
  type        = map(string)
  default     = {}
}

variable "enclave_secrets_manager_arns" {
  description = "Map of enclave env var name to Secrets Manager ARN."
  type        = map(string)
  default     = {}
}

variable "ingress_health_check_path" {
  description = "Health check path for API target group."
  type        = string
  default     = "/"
}

variable "alb_deletion_protection" {
  description = "Enable ALB deletion protection."
  type        = bool
  default     = false
}

variable "ingress_certificate_arn" {
  description = "ACM certificate ARN for HTTPS ingress."
  type        = string

  validation {
    condition     = trimspace(var.ingress_certificate_arn) != ""
    error_message = "ingress_certificate_arn must be set to a non-empty ACM certificate ARN."
  }
}

variable "ingress_ssl_policy" {
  description = "TLS policy for HTTPS listener."
  type        = string
  default     = "ELBSecurityPolicy-TLS13-1-2-2021-06"
}

variable "route53_zone_id" {
  description = "Optional Route53 hosted zone ID for creating API DNS record."
  type        = string
  default     = null
}

variable "route53_base_domain" {
  description = "Optional base domain for API record generation (for example: noderunner.wtf)."
  type        = string
  default     = null
}

variable "rds_db_name" {
  description = "Initial database name."
  type        = string
  default     = "alfred"
}

variable "rds_master_username" {
  description = "Master username for RDS."
  type        = string
  default     = "alfred"
}

variable "rds_instance_class" {
  description = "RDS instance class."
  type        = string
  default     = "db.t4g.micro"
}

variable "rds_engine_version" {
  description = "PostgreSQL engine version."
  type        = string
  default     = "16.3"
}

variable "rds_allocated_storage" {
  description = "Allocated storage in GB."
  type        = number
  default     = 20
}

variable "rds_max_allocated_storage" {
  description = "Max auto-scaled storage in GB."
  type        = number
  default     = 100
}

variable "rds_multi_az" {
  description = "Enable Multi-AZ for RDS."
  type        = bool
  default     = true
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
  description = "Number of cache nodes in Valkey replication group."
  type        = number
  default     = 1
}

variable "valkey_parameter_group_name" {
  description = "Valkey parameter group name."
  type        = string
  default     = "default.valkey7"
}

variable "enclave_ami_id" {
  description = "AMI ID for enclave parent host."
  type        = string
  default     = "ami-0f9fc25dd2506cf6d"
}

variable "enclave_instance_type" {
  description = "EC2 instance type for enclave parent host."
  type        = string
  default     = "c7i.large"
}

variable "enclave_key_name" {
  description = "Optional SSH key pair name for enclave host."
  type        = string
  default     = null
}

variable "enclave_root_volume_size" {
  description = "Root volume size (GB) for enclave host."
  type        = number
  default     = 30
}

variable "enclave_user_data" {
  description = "Optional user-data script for enclave host."
  type        = string
  default     = ""
}

variable "log_retention_days" {
  description = "CloudWatch log retention period in days."
  type        = number
  default     = 14
}

variable "create_alarms" {
  description = "Whether to create baseline CloudWatch alarms."
  type        = bool
  default     = true
}

variable "alarm_actions" {
  description = "Alarm action ARNs (for example SNS topic ARNs)."
  type        = list(string)
  default     = []
}

variable "kms_key_arns" {
  description = "Optional KMS key ARNs runtime tasks may decrypt with."
  type        = list(string)
  default     = []
}
