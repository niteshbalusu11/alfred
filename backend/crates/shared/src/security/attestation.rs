use base64::Engine as _;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

use crate::enclave_runtime::{AttestationChallengeResponse, attestation_signing_payload};

use super::SecurityError;

pub(crate) fn verify_attestation_signature(
    encoded_public_key: &str,
    encoded_signature: &str,
    response: &AttestationChallengeResponse,
) -> Result<(), SecurityError> {
    let public_key_bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded_public_key.as_bytes())
        .map_err(|_| SecurityError::InvalidAttestationPublicKey)?;
    let public_key_bytes: [u8; 32] = public_key_bytes
        .try_into()
        .map_err(|_| SecurityError::InvalidAttestationPublicKey)?;
    let public_key = VerifyingKey::from_bytes(&public_key_bytes)
        .map_err(|_| SecurityError::InvalidAttestationPublicKey)?;

    let signature_bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded_signature.as_bytes())
        .map_err(|_| SecurityError::InvalidAttestationSignature)?;
    let signature_bytes: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| SecurityError::InvalidAttestationSignature)?;
    let signature = Signature::try_from(&signature_bytes[..])
        .map_err(|_| SecurityError::InvalidAttestationSignature)?;

    public_key
        .verify(attestation_signing_payload(response).as_bytes(), &signature)
        .map_err(|_| SecurityError::InvalidAttestationSignature)?;

    Ok(())
}
