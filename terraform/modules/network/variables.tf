variable "name_prefix" {
  description = "Naming prefix for network resources."
  type        = string
}

variable "vpc_cidr" {
  description = "CIDR block for the VPC."
  type        = string
}

variable "availability_zones" {
  description = "Availability zones to use for subnets."
  type        = list(string)

  validation {
    condition     = length(var.availability_zones) > 0
    error_message = "availability_zones must include at least one AZ."
  }
}

variable "public_subnet_cidrs" {
  description = "Public subnet CIDR blocks aligned to availability_zones order."
  type        = list(string)

  validation {
    condition     = length(var.public_subnet_cidrs) == length(var.availability_zones)
    error_message = "public_subnet_cidrs length must match availability_zones length."
  }
}

variable "private_subnet_cidrs" {
  description = "Private subnet CIDR blocks aligned to availability_zones order."
  type        = list(string)

  validation {
    condition     = length(var.private_subnet_cidrs) == length(var.availability_zones)
    error_message = "private_subnet_cidrs length must match availability_zones length."
  }
}

variable "enable_nat_gateway" {
  description = "Whether to create a NAT gateway for private subnet egress."
  type        = bool
  default     = true
}
