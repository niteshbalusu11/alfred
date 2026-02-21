variable "aws_region" {
  description = "AWS region for the prod environment."
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
  description = "ACM certificate ARN for HTTPS ingress."
  type        = string
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

variable "rds_multi_az" {
  description = "Enable Multi-AZ for RDS."
  type        = bool
  default     = true
}
