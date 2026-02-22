variable "name_prefix" {
  description = "Naming prefix for RDS resources."
  type        = string
}

variable "subnet_ids" {
  description = "Private subnet IDs for the RDS subnet group."
  type        = list(string)
}

variable "security_group_id" {
  description = "Security group ID attached to RDS instance."
  type        = string
}

variable "db_name" {
  description = "Initial database name."
  type        = string
  default     = "alfred"
}

variable "master_username" {
  description = "Master username for the RDS instance."
  type        = string
  default     = "alfred"
}

variable "instance_class" {
  description = "RDS instance class."
  type        = string
  default     = "db.t4g.micro"
}

variable "engine_version" {
  description = "PostgreSQL engine version."
  type        = string
  default     = "18"
}

variable "allocated_storage" {
  description = "Allocated storage in GB."
  type        = number
  default     = 20
}

variable "max_allocated_storage" {
  description = "Maximum auto-scaled storage in GB."
  type        = number
  default     = 100
}

variable "multi_az" {
  description = "Enable Multi-AZ for RDS."
  type        = bool
  default     = false
}

variable "backup_retention_period" {
  description = "Automated backup retention in days."
  type        = number
  default     = 7
}

variable "deletion_protection" {
  description = "Enable deletion protection for RDS."
  type        = bool
  default     = false
}

variable "skip_final_snapshot" {
  description = "Skip final snapshot on destroy."
  type        = bool
  default     = true
}

variable "publicly_accessible" {
  description = "Whether the DB instance is publicly accessible."
  type        = bool
  default     = false
}

variable "storage_encrypted" {
  description = "Enable storage encryption."
  type        = bool
  default     = true
}
