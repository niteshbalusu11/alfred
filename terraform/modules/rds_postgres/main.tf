locals {
  identifier = "${var.name_prefix}-postgres"
}

resource "aws_db_subnet_group" "this" {
  name       = "${var.name_prefix}-db-subnets"
  subnet_ids = var.subnet_ids

  tags = {
    Name = "${var.name_prefix}-db-subnets"
  }
}

resource "aws_db_instance" "this" {
  identifier                      = local.identifier
  engine                          = "postgres"
  engine_version                  = var.engine_version
  instance_class                  = var.instance_class
  allocated_storage               = var.allocated_storage
  max_allocated_storage           = var.max_allocated_storage
  db_name                         = var.db_name
  username                        = var.master_username
  manage_master_user_password     = true
  db_subnet_group_name            = aws_db_subnet_group.this.name
  vpc_security_group_ids          = [var.security_group_id]
  multi_az                        = var.multi_az
  publicly_accessible             = var.publicly_accessible
  backup_retention_period         = var.backup_retention_period
  deletion_protection             = var.deletion_protection
  skip_final_snapshot             = var.skip_final_snapshot
  final_snapshot_identifier       = var.skip_final_snapshot ? null : "${local.identifier}-final"
  storage_encrypted               = var.storage_encrypted
  auto_minor_version_upgrade      = true
  performance_insights_enabled    = true
  enabled_cloudwatch_logs_exports = ["postgresql"]

  tags = {
    Name = local.identifier
  }
}
