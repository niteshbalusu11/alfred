use std::env;
use std::path::PathBuf;

use chrono::Utc;
use serde_json::{Value, json};
use shared::enclave_runtime::{AlfredEnvironment, EnclaveRuntimeMode};

#[derive(Debug, Clone)]
pub(crate) struct RuntimeConfig {
    pub(crate) bind_addr: String,
    pub(crate) environment: AlfredEnvironment,
    pub(crate) mode: EnclaveRuntimeMode,
    pub(crate) runtime_id: String,
    pub(crate) measurement: String,
    attestation_source: AttestationSource,
}

#[derive(Debug, Clone)]
enum AttestationSource {
    Inline(String),
    FilePath(PathBuf),
    Missing,
}

impl RuntimeConfig {
    pub(crate) fn from_env() -> Result<Self, String> {
        let environment = env::var("ALFRED_ENV")
            .unwrap_or_else(|_| "local".to_string())
            .parse::<AlfredEnvironment>()
            .map_err(|err| format!("invalid environment: {err}"))?;
        let default_mode = if matches!(environment, AlfredEnvironment::Local) {
            "dev-shim"
        } else {
            "remote"
        };
        let mode = env::var("ENCLAVE_RUNTIME_MODE")
            .unwrap_or_else(|_| default_mode.to_string())
            .parse::<EnclaveRuntimeMode>()
            .map_err(|err| format!("invalid enclave runtime mode: {err}"))?;
        let runtime_id = env::var("TEE_EXPECTED_RUNTIME").unwrap_or_else(|_| "nitro".to_string());
        let measurement = env::var("ENCLAVE_RUNTIME_MEASUREMENT")
            .unwrap_or_else(|_| "dev-local-enclave".to_string());
        let attestation_source = if let Ok(path) = env::var("TEE_ATTESTATION_DOCUMENT_PATH") {
            AttestationSource::FilePath(PathBuf::from(path))
        } else if let Ok(document) = env::var("TEE_ATTESTATION_DOCUMENT") {
            AttestationSource::Inline(document)
        } else {
            AttestationSource::Missing
        };

        if matches!(mode, EnclaveRuntimeMode::Disabled) {
            return Err(
                "ENCLAVE_RUNTIME_MODE=disabled is invalid for enclave-runtime process".to_string(),
            );
        }

        if !matches!(environment, AlfredEnvironment::Local)
            && matches!(mode, EnclaveRuntimeMode::DevShim)
        {
            return Err(
                "ENCLAVE_RUNTIME_MODE=dev-shim is only allowed when ALFRED_ENV=local".to_string(),
            );
        }

        if matches!(mode, EnclaveRuntimeMode::Remote)
            && matches!(attestation_source, AttestationSource::Missing)
        {
            return Err(
                "remote mode requires TEE_ATTESTATION_DOCUMENT_PATH or TEE_ATTESTATION_DOCUMENT"
                    .to_string(),
            );
        }

        Ok(Self {
            bind_addr: env::var("ENCLAVE_RUNTIME_BIND_ADDR")
                .unwrap_or_else(|_| "127.0.0.1:8181".to_string()),
            environment,
            mode,
            runtime_id,
            measurement,
            attestation_source,
        })
    }

    pub(crate) fn attestation_document(&self) -> Result<Value, String> {
        if matches!(self.mode, EnclaveRuntimeMode::DevShim) {
            return Ok(json!({
                "runtime": self.runtime_id,
                "measurement": self.measurement,
                "issued_at": Utc::now().timestamp(),
                "signature": null,
                "dev_shim": true
            }));
        }

        let raw = match &self.attestation_source {
            AttestationSource::Inline(document) => document.to_string(),
            AttestationSource::FilePath(path) => std::fs::read_to_string(path)
                .map_err(|err| format!("failed to read attestation document: {err}"))?,
            AttestationSource::Missing => {
                return Err(
                    "attestation document is missing for remote enclave runtime mode".to_string(),
                );
            }
        };

        serde_json::from_str::<Value>(&raw)
            .map_err(|err| format!("failed to parse attestation document: {err}"))
    }
}

#[cfg(test)]
mod tests {
    use shared::enclave_runtime::{AlfredEnvironment, EnclaveRuntimeMode};

    use super::{AttestationSource, RuntimeConfig};

    #[test]
    fn dev_shim_attestation_document_is_generated() {
        let config = RuntimeConfig {
            bind_addr: "127.0.0.1:8181".to_string(),
            environment: AlfredEnvironment::Local,
            mode: EnclaveRuntimeMode::DevShim,
            runtime_id: "nitro".to_string(),
            measurement: "dev-local-enclave".to_string(),
            attestation_source: AttestationSource::Missing,
        };

        let document = config
            .attestation_document()
            .expect("dev-shim document should be generated");
        assert_eq!(document["runtime"], "nitro");
        assert_eq!(document["measurement"], "dev-local-enclave");
        assert_eq!(document["dev_shim"], true);
    }

    #[test]
    fn remote_attestation_document_fails_when_source_missing() {
        let config = RuntimeConfig {
            bind_addr: "127.0.0.1:8181".to_string(),
            environment: AlfredEnvironment::Local,
            mode: EnclaveRuntimeMode::Remote,
            runtime_id: "nitro".to_string(),
            measurement: "mr_enclave_1".to_string(),
            attestation_source: AttestationSource::Missing,
        };

        let err = config
            .attestation_document()
            .expect_err("missing source should fail");
        assert!(
            err.contains("attestation document is missing"),
            "unexpected error message: {err}"
        );
    }
}
