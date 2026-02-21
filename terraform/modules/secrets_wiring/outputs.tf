output "api_ecs_secrets" {
  description = "ECS secrets block entries for API container definitions."
  value       = concat(local.api_ssm_secrets, local.api_sm_secrets)
}

output "worker_ecs_secrets" {
  description = "ECS secrets block entries for worker container definitions."
  value       = concat(local.worker_ssm_secrets, local.worker_sm_secrets)
}

output "all_ssm_secret_arns" {
  description = "All SSM parameter ARNs referenced by runtime wiring."
  value = concat(
    values(var.api_ssm_secret_arns),
    values(var.worker_ssm_secret_arns),
    local.enclave_ssm_secret_values
  )
}

output "all_secrets_manager_arns" {
  description = "All Secrets Manager ARNs referenced by runtime wiring."
  value = concat(
    values(var.api_secrets_manager_arns),
    values(var.worker_secrets_manager_arns),
    local.enclave_sm_secret_values
  )
}

output "enclave_secret_arns" {
  description = "Combined secret ARNs for enclave host runtime wiring."
  value       = concat(local.enclave_ssm_secret_values, local.enclave_sm_secret_values)
}
