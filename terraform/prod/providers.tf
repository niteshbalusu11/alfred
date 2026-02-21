provider "aws" {
  region = var.aws_region

  default_tags {
    tags = merge(
      {
        project     = "alfred"
        environment = "prod"
        managed_by  = "terraform"
      },
      var.additional_tags
    )
  }
}
