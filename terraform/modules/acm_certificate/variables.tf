variable "domain_name" {
  description = "Primary domain name for certificate."
  type        = string
}

variable "subject_alternative_names" {
  description = "Optional SAN entries for certificate."
  type        = list(string)
  default     = []
}

variable "zone_id" {
  description = "Route53 hosted zone ID for DNS validation records."
  type        = string
}

variable "tags" {
  description = "Tags applied to ACM certificate."
  type        = map(string)
  default     = {}
}
