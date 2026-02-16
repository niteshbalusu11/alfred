use std::env;

use crate::config::ConfigError;
use crate::enclave_runtime::{AlfredEnvironment, EnclaveRuntimeMode};

pub(crate) fn parse_alfred_environment() -> Result<AlfredEnvironment, ConfigError> {
    env::var("ALFRED_ENV")
        .unwrap_or_else(|_| "local".to_string())
        .parse::<AlfredEnvironment>()
        .map_err(ConfigError::InvalidConfiguration)
}

pub(crate) fn parse_enclave_runtime_mode(
    key: &str,
    _alfred_environment: AlfredEnvironment,
) -> Result<EnclaveRuntimeMode, ConfigError> {
    env::var(key)
        .unwrap_or_else(|_| "remote".to_string())
        .parse::<EnclaveRuntimeMode>()
        .map_err(ConfigError::InvalidConfiguration)
}

pub(crate) fn validate_enclave_runtime_guards(
    alfred_environment: AlfredEnvironment,
    enclave_runtime_mode: EnclaveRuntimeMode,
    tee_attestation_required: bool,
    tee_allow_insecure_dev_attestation: bool,
) -> Result<(), ConfigError> {
    if !matches!(alfred_environment, AlfredEnvironment::Local)
        && matches!(
            enclave_runtime_mode,
            EnclaveRuntimeMode::DevShim | EnclaveRuntimeMode::Disabled
        )
    {
        return Err(ConfigError::InvalidConfiguration(
            "ENCLAVE_RUNTIME_MODE must be remote in staging/production environments".to_string(),
        ));
    }

    if matches!(enclave_runtime_mode, EnclaveRuntimeMode::DevShim)
        && (tee_attestation_required || !tee_allow_insecure_dev_attestation)
    {
        return Err(ConfigError::InvalidConfiguration(
            "dev-shim enclave mode requires TEE_ATTESTATION_REQUIRED=false and TEE_ALLOW_INSECURE_DEV_ATTESTATION=true"
                .to_string(),
        ));
    }

    Ok(())
}
