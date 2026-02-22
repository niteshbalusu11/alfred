locals {
  replication_group_id = "${var.name_prefix}-valkey"
  enable_failover      = var.num_cache_clusters > 1
}

resource "aws_elasticache_subnet_group" "this" {
  name       = "${var.name_prefix}-cache-subnets"
  subnet_ids = var.subnet_ids
}

resource "aws_elasticache_replication_group" "this" {
  replication_group_id       = local.replication_group_id
  description                = "Valkey replication group for ${var.name_prefix}"
  engine                     = "valkey"
  engine_version             = var.engine_version
  node_type                  = var.node_type
  port                       = var.port
  parameter_group_name       = var.parameter_group_name
  num_cache_clusters         = var.num_cache_clusters
  subnet_group_name          = aws_elasticache_subnet_group.this.name
  security_group_ids         = [var.security_group_id]
  automatic_failover_enabled = local.enable_failover
  multi_az_enabled           = local.enable_failover
  at_rest_encryption_enabled = var.at_rest_encryption_enabled
  transit_encryption_enabled = var.transit_encryption_enabled
  apply_immediately          = true

  tags = {
    Name = local.replication_group_id
  }
}
