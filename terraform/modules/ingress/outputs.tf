output "alb_arn" {
  description = "ALB ARN."
  value       = aws_lb.api.arn
}

output "alb_arn_suffix" {
  description = "ALB ARN suffix used in CloudWatch metrics dimensions."
  value       = aws_lb.api.arn_suffix
}

output "alb_dns_name" {
  description = "Public ALB DNS name."
  value       = aws_lb.api.dns_name
}

output "alb_zone_id" {
  description = "Route53 zone ID for the ALB."
  value       = aws_lb.api.zone_id
}

output "target_group_arn" {
  description = "API target group ARN."
  value       = aws_lb_target_group.api.arn
}

output "target_group_arn_suffix" {
  description = "Target group ARN suffix used in CloudWatch metrics dimensions."
  value       = aws_lb_target_group.api.arn_suffix
}

output "listener_arn" {
  description = "Preferred listener ARN (HTTPS when configured, otherwise HTTP)."
  value = coalesce(
    try(aws_lb_listener.https[0].arn, null),
    try(aws_lb_listener.http_redirect[0].arn, null),
    try(aws_lb_listener.http_forward[0].arn, null)
  )
}

output "http_listener_arn" {
  description = "HTTP listener ARN when configured."
  value = coalesce(
    try(aws_lb_listener.http_redirect[0].arn, null),
    try(aws_lb_listener.http_forward[0].arn, null)
  )
}

output "https_listener_arn" {
  description = "HTTPS listener ARN when configured."
  value       = try(aws_lb_listener.https[0].arn, null)
}
