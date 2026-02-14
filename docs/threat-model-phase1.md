# Phase I Threat Model (TEE + KMS Decrypt Path)

- Last Updated: 2026-02-14
- Scope: Connector refresh-token decrypt and Google API access path

## 1) Assets

1. Google refresh tokens in `connectors.refresh_token_ciphertext`.
2. KMS decrypt permission bound to Alfred key policy.
3. Attestation evidence describing enclave runtime identity and measurement.
4. Attestation verification key used to validate evidence signatures.
5. Runtime-refreshed attestation document source (`TEE_ATTESTATION_DOCUMENT_PATH`).

## 2) Trust Boundaries

1. API host process is untrusted for plaintext refresh tokens.
2. Attested enclave runtime is trusted to request decrypt only when:
   1. Runtime matches expected TEE platform.
   2. Measurement is in approved allow-list.
   3. Evidence signature is valid and evidence timestamp is fresh.
3. KMS decrypt policy is trusted to enforce measurement + key constraints.

## 3) Primary Threats and Mitigations

1. DB ciphertext exfiltration:
   1. Mitigation: decrypt requires valid attestation and matching KMS key metadata.
2. Host-level compromise attempts token decrypt:
   1. Mitigation: runtime/measurement checks plus signed attestation verification deny decrypt before token retrieval.
3. Key confusion during rotation:
   1. Mitigation: connector rows persist `token_key_id` and `token_version`; decrypt requires exact match.
4. Legacy row migration lockout:
   1. Mitigation: legacy connector rows are marked with `__legacy__` and adopted to active key-id + key-version before strict checks.
5. Sensitive value leakage in logs:
   1. Mitigation: token plaintext never logged; denial paths log only policy reasons.

## 4) Decrypt Authorization Contract

Decrypt is authorized only when all conditions pass:

1. Connector exists and is `ACTIVE`.
2. Persisted connector metadata (`token_key_id`, `token_version`) matches active KMS policy.
3. Attestation document is valid JSON and runtime matches expected TEE runtime.
4. Attestation signature validates against trusted attestation public key.
5. Attestation timestamp is within freshness window.
6. Attested measurement is allow-listed in TEE and KMS policies.

Failure in any condition must deny decrypt.
