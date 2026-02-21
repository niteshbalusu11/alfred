locals {
  api_ssm_secrets = [
    for name, arn in var.api_ssm_secret_arns : {
      name      = name
      valueFrom = arn
    }
  ]

  api_sm_secrets = [
    for name, arn in var.api_secrets_manager_arns : {
      name      = name
      valueFrom = arn
    }
  ]

  worker_ssm_secrets = [
    for name, arn in var.worker_ssm_secret_arns : {
      name      = name
      valueFrom = arn
    }
  ]

  worker_sm_secrets = [
    for name, arn in var.worker_secrets_manager_arns : {
      name      = name
      valueFrom = arn
    }
  ]

  enclave_ssm_secret_values = values(var.enclave_ssm_secret_arns)
  enclave_sm_secret_values  = values(var.enclave_secrets_manager_arns)
}
