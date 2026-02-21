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
