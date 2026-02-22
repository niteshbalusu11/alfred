variable "name_prefix" {
  description = "Naming prefix for ElastiCache resources."
  type        = string
}

variable "subnet_ids" {
  description = "Private subnet IDs for the ElastiCache subnet group."
  type        = list(string)
}

variable "security_group_id" {
  description = "Security group ID attached to Valkey replication group."
  type        = string
}

variable "node_type" {
  description = "Cache node type for Valkey."
  type        = string
  default     = "cache.t4g.micro"
}

variable "num_cache_clusters" {
  description = "Number of cache clusters in the replication group."
  type        = number
  default     = 1
}

variable "engine_version" {
  description = "Valkey engine version."
  type        = string
  default     = "8.2"
}

variable "port" {
  description = "Valkey listener port."
  type        = number
  default     = 6379
}

variable "parameter_group_name" {
  description = "Valkey parameter group name."
  type        = string
  default     = "default.valkey8"
}

variable "at_rest_encryption_enabled" {
  description = "Enable at-rest encryption."
  type        = bool
  default     = true
}

variable "transit_encryption_enabled" {
  description = "Enable in-transit encryption."
  type        = bool
  default     = true
}
