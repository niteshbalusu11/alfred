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
