output "api_fqdn" {
  description = "Fully-qualified domain name for API ingress."
  value       = aws_route53_record.api_alias.fqdn
}

output "api_record_name" {
  description = "Configured Route53 record name."
  value       = aws_route53_record.api_alias.name
}
