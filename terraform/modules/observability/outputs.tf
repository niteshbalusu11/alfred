output "api_log_group_name" {
  description = "API log group name."
  value       = aws_cloudwatch_log_group.api.name
}

output "worker_log_group_name" {
  description = "Worker log group name."
  value       = aws_cloudwatch_log_group.worker.name
}

output "enclave_log_group_name" {
  description = "Enclave host log group name."
  value       = aws_cloudwatch_log_group.enclave.name
}

output "alarm_names" {
  description = "Created CloudWatch alarm names."
  value = compact([
    try(aws_cloudwatch_metric_alarm.api_alb_5xx[0].alarm_name, null),
    try(aws_cloudwatch_metric_alarm.api_ecs_cpu[0].alarm_name, null),
    try(aws_cloudwatch_metric_alarm.rds_cpu[0].alarm_name, null),
    try(aws_cloudwatch_metric_alarm.valkey_cpu[0].alarm_name, null)
  ])
}
