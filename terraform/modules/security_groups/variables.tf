variable "name_prefix" {
  description = "Naming prefix for security groups."
  type        = string
}

variable "vpc_id" {
  description = "VPC identifier."
  type        = string
}

variable "api_port" {
  description = "API container port exposed behind ALB."
  type        = number
  default     = 8080
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

variable "enable_http_ingress" {
  description = "Allow public HTTP ingress to ALB."
  type        = bool
  default     = true
}

variable "enable_https_ingress" {
  description = "Allow public HTTPS ingress to ALB."
  type        = bool
  default     = false
}
