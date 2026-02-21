locals {
  stack_name = "${var.name_prefix}-${var.environment}"

  default_tags = {
    project     = "alfred"
    environment = var.environment
    managed_by  = "terraform"
  }

  effective_tags = merge(local.default_tags, var.tags)
}
