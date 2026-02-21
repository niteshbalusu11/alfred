variable "name_prefix" {
  description = "Naming prefix for enclave parent host resources."
  type        = string
}

variable "subnet_id" {
  description = "Private subnet ID for enclave parent host."
  type        = string
}

variable "security_group_id" {
  description = "Security group ID for enclave parent host."
  type        = string
}

variable "ami_id" {
  description = "Optional AMI ID for enclave parent host. If unset, module resolves latest Amazon Linux AMI."
  type        = string
  default     = null
}

variable "instance_type" {
  description = "EC2 instance type for enclave parent host."
  type        = string
  default     = "c7i.large"
}

variable "iam_instance_profile_name" {
  description = "IAM instance profile name attached to enclave parent host."
  type        = string
}

variable "key_name" {
  description = "Optional SSH key pair name."
  type        = string
  default     = null
}

variable "root_volume_size" {
  description = "Root EBS volume size in GB."
  type        = number
  default     = 30
}

variable "user_data" {
  description = "Optional user data for instance bootstrap."
  type        = string
  default     = ""
}
