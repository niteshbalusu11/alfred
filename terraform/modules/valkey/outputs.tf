output "replication_group_id" {
  description = "Valkey replication group ID."
  value       = aws_elasticache_replication_group.this.replication_group_id
}

output "primary_endpoint" {
  description = "Primary endpoint for Valkey writes."
  value       = aws_elasticache_replication_group.this.primary_endpoint_address
}

output "reader_endpoint" {
  description = "Reader endpoint for Valkey reads."
  value       = aws_elasticache_replication_group.this.reader_endpoint_address
}

output "port" {
  description = "Valkey endpoint port."
  value       = aws_elasticache_replication_group.this.port
}
