name_prefix = "alfred"

additional_tags = {
  owner = "alfred-prod"
}

# Prod safety defaults.
rds_deletion_protection = true
rds_skip_final_snapshot = false
rds_multi_az            = true

# Prod should use TLS at ingress. Replace with real ACM cert ARN before apply.
ingress_certificate_arn = "arn:aws:acm:us-east-2:123456789012:certificate/replace-me"
route53_zone_id         = "Z10154612GBUAYQKQMWC3"
route53_base_domain     = "noderunner.wtf"
