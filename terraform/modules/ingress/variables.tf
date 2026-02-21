variable "name_prefix" {
  description = "Naming prefix for ingress resources."
  type        = string
}

variable "vpc_id" {
  description = "VPC identifier."
  type        = string
}

variable "public_subnet_ids" {
  description = "Public subnet IDs for internet-facing ALB."
  type        = list(string)
}

variable "security_group_id" {
  description = "ALB security group ID."
  type        = string
}

variable "target_port" {
  description = "Target port on API tasks."
  type        = number
  default     = 8080
}

variable "health_check_path" {
  description = "Health check path for API target group."
  type        = string
  default     = "/health"
}

variable "deletion_protection" {
  description = "Enable ALB deletion protection."
  type        = bool
  default     = false
}

variable "certificate_arn" {
  description = "Optional ACM certificate ARN for HTTPS listener."
  type        = string
  default     = null
}

variable "enable_http_listener" {
  description = "Whether to expose HTTP listener on port 80."
  type        = bool
  default     = true
}

variable "ssl_policy" {
  description = "SSL policy for HTTPS listener."
  type        = string
  default     = "ELBSecurityPolicy-TLS13-1-2-2021-06"
}
