locals {
  enable_alb_alarm    = var.create_alarms && var.alb_arn_suffix != null && var.target_group_arn_suffix != null
  enable_ecs_alarm    = var.create_alarms && var.ecs_cluster_name != null && var.api_service_name != null
  enable_rds_alarm    = var.create_alarms && var.rds_identifier != null
  enable_valkey_alarm = var.create_alarms && var.valkey_replication_group_id != null
}

resource "aws_cloudwatch_log_group" "api" {
  name              = var.api_log_group_name
  retention_in_days = var.log_retention_days
}

resource "aws_cloudwatch_log_group" "worker" {
  name              = var.worker_log_group_name
  retention_in_days = var.log_retention_days
}

resource "aws_cloudwatch_log_group" "enclave" {
  name              = var.enclave_log_group_name
  retention_in_days = var.log_retention_days
}

resource "aws_cloudwatch_metric_alarm" "api_alb_5xx" {
  count = local.enable_alb_alarm ? 1 : 0

  alarm_name          = "${var.name_prefix}-api-alb-5xx"
  comparison_operator = "GreaterThanOrEqualToThreshold"
  evaluation_periods  = 2
  metric_name         = "HTTPCode_Target_5XX_Count"
  namespace           = "AWS/ApplicationELB"
  period              = 60
  statistic           = "Sum"
  threshold           = 5
  alarm_description   = "ALB target 5xx responses are elevated"
  treat_missing_data  = "notBreaching"
  alarm_actions       = var.alarm_actions
  ok_actions          = var.alarm_actions

  dimensions = {
    LoadBalancer = var.alb_arn_suffix
    TargetGroup  = var.target_group_arn_suffix
  }
}

resource "aws_cloudwatch_metric_alarm" "api_ecs_cpu" {
  count = local.enable_ecs_alarm ? 1 : 0

  alarm_name          = "${var.name_prefix}-api-ecs-cpu-high"
  comparison_operator = "GreaterThanOrEqualToThreshold"
  evaluation_periods  = 3
  metric_name         = "CPUUtilization"
  namespace           = "AWS/ECS"
  period              = 60
  statistic           = "Average"
  threshold           = var.api_cpu_alarm_threshold
  alarm_description   = "API ECS service CPU utilization is high"
  treat_missing_data  = "notBreaching"
  alarm_actions       = var.alarm_actions
  ok_actions          = var.alarm_actions

  dimensions = {
    ClusterName = var.ecs_cluster_name
    ServiceName = var.api_service_name
  }
}

resource "aws_cloudwatch_metric_alarm" "rds_cpu" {
  count = local.enable_rds_alarm ? 1 : 0

  alarm_name          = "${var.name_prefix}-rds-cpu-high"
  comparison_operator = "GreaterThanOrEqualToThreshold"
  evaluation_periods  = 3
  metric_name         = "CPUUtilization"
  namespace           = "AWS/RDS"
  period              = 60
  statistic           = "Average"
  threshold           = var.rds_cpu_alarm_threshold
  alarm_description   = "RDS CPU utilization is high"
  treat_missing_data  = "notBreaching"
  alarm_actions       = var.alarm_actions
  ok_actions          = var.alarm_actions

  dimensions = {
    DBInstanceIdentifier = var.rds_identifier
  }
}

resource "aws_cloudwatch_metric_alarm" "valkey_cpu" {
  count = local.enable_valkey_alarm ? 1 : 0

  alarm_name          = "${var.name_prefix}-valkey-cpu-high"
  comparison_operator = "GreaterThanOrEqualToThreshold"
  evaluation_periods  = 3
  metric_name         = "EngineCPUUtilization"
  namespace           = "AWS/ElastiCache"
  period              = 60
  statistic           = "Average"
  threshold           = var.valkey_cpu_alarm_threshold
  alarm_description   = "Valkey engine CPU utilization is high"
  treat_missing_data  = "notBreaching"
  alarm_actions       = var.alarm_actions
  ok_actions          = var.alarm_actions

  dimensions = {
    ReplicationGroupId = var.valkey_replication_group_id
  }
}
