variable "name_prefix" {
  description = "Naming prefix for IAM resources."
  type        = string
}

variable "ssm_parameter_arns" {
  description = "SSM parameter ARNs readable by runtime task roles."
  type        = list(string)
  default     = []
}

variable "secrets_manager_arns" {
  description = "Secrets Manager ARNs readable by runtime task roles."
  type        = list(string)
  default     = []
}
