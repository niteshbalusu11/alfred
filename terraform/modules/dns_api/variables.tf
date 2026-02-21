variable "zone_id" {
  description = "Route53 hosted zone ID for public DNS records."
  type        = string
}

variable "record_name" {
  description = "FQDN for the API alias record."
  type        = string
}

variable "alb_dns_name" {
  description = "ALB DNS name target for Route53 alias."
  type        = string
}

variable "alb_zone_id" {
  description = "ALB hosted zone ID required for Route53 alias."
  type        = string
}
