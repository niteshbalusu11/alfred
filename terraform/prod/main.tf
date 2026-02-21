module "environment" {
  source = "../modules"

  environment = "prod"
  aws_region  = var.aws_region
  name_prefix = var.name_prefix
  tags        = var.additional_tags

  ingress_certificate_arn = var.ingress_certificate_arn
  rds_deletion_protection = var.rds_deletion_protection
  rds_skip_final_snapshot = var.rds_skip_final_snapshot
  rds_multi_az            = var.rds_multi_az
}
