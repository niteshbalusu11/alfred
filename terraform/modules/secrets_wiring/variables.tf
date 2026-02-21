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
