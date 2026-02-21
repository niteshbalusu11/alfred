module "environment" {
  source = "../modules"

  environment = "prod"
  aws_region  = var.aws_region
  name_prefix = var.name_prefix
  tags        = var.additional_tags
}
