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
