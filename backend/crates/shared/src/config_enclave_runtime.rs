use std::env;

use crate::config::ConfigError;
use crate::enclave_runtime::{AlfredEnvironment, EnclaveRuntimeMode};

pub(crate) fn parse_alfred_environment() -> Result<AlfredEnvironment, ConfigError> {
    env::var("ALFRED_ENV")
        .unwrap_or_else(|_| "production".to_string())
        .parse::<AlfredEnvironment>()
        .map_err(ConfigError::InvalidConfiguration)
}

pub(crate) fn parse_enclave_runtime_mode(key: &str) -> Result<EnclaveRuntimeMode, ConfigError> {
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

pub(crate) fn validate_non_local_enclave_security_posture(
    alfred_environment: AlfredEnvironment,
    tee_attestation_required: bool,
    tee_allow_insecure_dev_attestation: bool,
    tee_allowed_measurements: &[String],
    kms_allowed_measurements: &[String],
    enclave_runtime_base_url: &str,
) -> Result<(), ConfigError> {
    if matches!(alfred_environment, AlfredEnvironment::Local) {
        return Ok(());
    }

    if !tee_attestation_required {
        return Err(ConfigError::InvalidConfiguration(
            "TEE_ATTESTATION_REQUIRED must be true outside local environment".to_string(),
        ));
    }

    if tee_allow_insecure_dev_attestation {
        return Err(ConfigError::InvalidConfiguration(
            "TEE_ALLOW_INSECURE_DEV_ATTESTATION must be false outside local environment"
                .to_string(),
        ));
    }

    validate_measurement_allowlist("TEE_ALLOWED_MEASUREMENTS", tee_allowed_measurements)?;
    validate_measurement_allowlist("KMS_ALLOWED_MEASUREMENTS", kms_allowed_measurements)?;
    validate_non_local_runtime_base_url(enclave_runtime_base_url)?;

    Ok(())
}

fn validate_measurement_allowlist(key: &str, measurements: &[String]) -> Result<(), ConfigError> {
    if measurements.is_empty() {
        return Err(ConfigError::InvalidConfiguration(format!(
            "{key} must contain at least one non-dev measurement outside local environment"
        )));
    }

    if measurements
        .iter()
        .any(|measurement| measurement == "dev-local-enclave")
    {
        return Err(ConfigError::InvalidConfiguration(format!(
            "{key} must not include dev-local-enclave outside local environment"
        )));
    }

    Ok(())
}

fn validate_non_local_runtime_base_url(base_url: &str) -> Result<(), ConfigError> {
    let parsed = reqwest::Url::parse(base_url).map_err(|_| {
        ConfigError::InvalidConfiguration(
            "ENCLAVE_RUNTIME_BASE_URL must be a valid URL".to_string(),
        )
    })?;
    if parsed.scheme() == "https" {
        return Ok(());
    }

    if parsed.scheme() == "http"
        && matches!(
            parsed.host_str(),
            Some("127.0.0.1") | Some("localhost") | Some("::1")
        )
    {
        return Ok(());
    }

    Err(ConfigError::InvalidConfiguration(
        "ENCLAVE_RUNTIME_BASE_URL must use https outside local environment unless it is loopback http".to_string(),
    ))
}

pub(crate) fn parse_enclave_rpc_shared_secret(
    environment: AlfredEnvironment,
) -> Result<String, ConfigError> {
    if let Ok(value) = env::var("ENCLAVE_RPC_SHARED_SECRET") {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            return Err(ConfigError::MissingVar(
                "ENCLAVE_RPC_SHARED_SECRET".to_string(),
            ));
        }
        if trimmed.len() < 16 {
            return Err(ConfigError::InvalidConfiguration(
                "ENCLAVE_RPC_SHARED_SECRET must be at least 16 characters".to_string(),
            ));
        }
        return Ok(trimmed);
    }

    if matches!(environment, AlfredEnvironment::Local) {
        return Ok("local-dev-enclave-rpc-secret".to_string());
    }

    Err(ConfigError::MissingVar(
        "ENCLAVE_RPC_SHARED_SECRET".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::{validate_non_local_enclave_security_posture, validate_non_local_runtime_base_url};
    use crate::enclave_runtime::AlfredEnvironment;

    fn prod_measurements() -> Vec<String> {
        vec!["mr-enclave-prod-a".to_string()]
    }

    #[test]
    fn non_local_rejects_disabled_attestation() {
        let err = validate_non_local_enclave_security_posture(
            AlfredEnvironment::Production,
            false,
            false,
            &prod_measurements(),
            &prod_measurements(),
            "https://enclave.internal",
        )
        .expect_err("non-local should fail when attestation is disabled");

        assert!(err.to_string().contains("TEE_ATTESTATION_REQUIRED"));
    }

    #[test]
    fn non_local_rejects_insecure_dev_attestation() {
        let err = validate_non_local_enclave_security_posture(
            AlfredEnvironment::Staging,
            true,
            true,
            &prod_measurements(),
            &prod_measurements(),
            "https://enclave.internal",
        )
        .expect_err("non-local should fail when insecure attestation mode is enabled");

        assert!(
            err.to_string()
                .contains("TEE_ALLOW_INSECURE_DEV_ATTESTATION")
        );
    }

    #[test]
    fn non_local_rejects_dev_measurement_entries() {
        let err = validate_non_local_enclave_security_posture(
            AlfredEnvironment::Production,
            true,
            false,
            &["dev-local-enclave".to_string()],
            &prod_measurements(),
            "https://enclave.internal",
        )
        .expect_err("non-local should reject dev measurements");

        assert!(err.to_string().contains("TEE_ALLOWED_MEASUREMENTS"));
    }

    #[test]
    fn non_local_rejects_non_loopback_http_runtime_url() {
        let err = validate_non_local_runtime_base_url("http://enclave.internal:8181")
            .expect_err("non-loopback http should fail outside local");

        assert!(err.to_string().contains("ENCLAVE_RUNTIME_BASE_URL"));
    }

    #[test]
    fn non_local_accepts_https_or_loopback_http_runtime_url() {
        validate_non_local_runtime_base_url("https://enclave.internal:8181")
            .expect("https runtime URL should pass");
        validate_non_local_runtime_base_url("http://127.0.0.1:8181")
            .expect("loopback runtime URL should pass");
    }
}
