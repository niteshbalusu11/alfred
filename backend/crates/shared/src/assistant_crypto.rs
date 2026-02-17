use base64::Engine as _;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use sha2::{Digest, Sha256};
use thiserror::Error;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::models::{
    AssistantEncryptedRequestEnvelope, AssistantEncryptedResponseEnvelope,
    AssistantPlaintextQueryRequest, AssistantPlaintextQueryResponse,
};

pub const ASSISTANT_ENVELOPE_VERSION_V1: &str = "v1";
pub const ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305: &str = "x25519-chacha20poly1305";

#[derive(Debug, Clone)]
pub struct AssistantIngressKeyMaterial {
    pub key_id: String,
    pub private_key: [u8; 32],
    pub public_key: String,
    pub key_expires_at: i64,
}

#[derive(Debug, Clone)]
pub struct AssistantIngressKeyring {
    pub active: AssistantIngressKeyMaterial,
    pub previous: Option<AssistantIngressKeyMaterial>,
}

impl AssistantIngressKeyring {
    pub fn key_for_id(&self, key_id: &str) -> Option<&AssistantIngressKeyMaterial> {
        if self.active.key_id == key_id {
            return Some(&self.active);
        }

        self.previous.as_ref().filter(|key| key.key_id == key_id)
    }
}

#[derive(Debug, Error)]
pub enum AssistantCryptoError {
    #[error("assistant envelope version is unsupported")]
    UnsupportedVersion,
    #[error("assistant envelope algorithm is unsupported")]
    UnsupportedAlgorithm,
    #[error("assistant request_id is required")]
    MissingRequestId,
    #[error("assistant ingress key_id is not recognized")]
    UnknownKeyId,
    #[error("assistant ingress key_id has expired")]
    ExpiredKeyId,
    #[error("assistant envelope field is invalid base64: {field}")]
    InvalidBase64Field { field: &'static str },
    #[error("assistant envelope nonce must be exactly 12 bytes")]
    InvalidNonceLength,
    #[error("assistant public key is invalid")]
    InvalidPublicKey,
    #[error("assistant ciphertext failed authentication")]
    DecryptFailed,
    #[error("assistant plaintext payload is invalid: {0}")]
    InvalidPlaintextPayload(String),
    #[error("assistant response encryption failed")]
    EncryptFailed,
}

pub fn decrypt_assistant_request(
    keyring: &AssistantIngressKeyring,
    envelope: &AssistantEncryptedRequestEnvelope,
) -> Result<(AssistantPlaintextQueryRequest, AssistantIngressKeyMaterial), AssistantCryptoError> {
    validate_common_envelope_fields(
        envelope.version.as_str(),
        envelope.algorithm.as_str(),
        envelope.request_id.as_str(),
    )?;

    let key = keyring
        .key_for_id(envelope.key_id.as_str())
        .ok_or(AssistantCryptoError::UnknownKeyId)?
        .clone();
    let is_active_key = key.key_id == keyring.active.key_id;
    if !is_active_key && key.key_expires_at < chrono::Utc::now().timestamp() {
        return Err(AssistantCryptoError::ExpiredKeyId);
    }

    let client_public_key_bytes = decode_base64_field(
        envelope.client_ephemeral_public_key.as_str(),
        "client_ephemeral_public_key",
    )?;
    let client_public_key_bytes: [u8; 32] = client_public_key_bytes
        .try_into()
        .map_err(|_| AssistantCryptoError::InvalidPublicKey)?;
    let client_public_key = PublicKey::from(client_public_key_bytes);

    let nonce_bytes = decode_base64_field(envelope.nonce.as_str(), "nonce")?;
    if nonce_bytes.len() != 12 {
        return Err(AssistantCryptoError::InvalidNonceLength);
    }

    let ciphertext = decode_base64_field(envelope.ciphertext.as_str(), "ciphertext")?;
    let decrypt_key = derive_directional_key(
        key.private_key,
        client_public_key,
        envelope.request_id.as_str(),
        b"request",
    );

    let cipher = ChaCha20Poly1305::new_from_slice(&decrypt_key)
        .map_err(|_| AssistantCryptoError::DecryptFailed)?;
    let plaintext = cipher
        .decrypt(
            Nonce::from_slice(&nonce_bytes),
            Payload {
                msg: ciphertext.as_slice(),
                aad: envelope.request_id.as_bytes(),
            },
        )
        .map_err(|_| AssistantCryptoError::DecryptFailed)?;

    let parsed = serde_json::from_slice::<AssistantPlaintextQueryRequest>(&plaintext)
        .map_err(|err| AssistantCryptoError::InvalidPlaintextPayload(err.to_string()))?;

    Ok((parsed, key))
}

pub fn encrypt_assistant_response(
    key: &AssistantIngressKeyMaterial,
    request_id: &str,
    client_ephemeral_public_key_b64: &str,
    response: &AssistantPlaintextQueryResponse,
) -> Result<AssistantEncryptedResponseEnvelope, AssistantCryptoError> {
    validate_common_envelope_fields(
        ASSISTANT_ENVELOPE_VERSION_V1,
        ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305,
        request_id,
    )?;

    let client_public_key_bytes = decode_base64_field(
        client_ephemeral_public_key_b64,
        "client_ephemeral_public_key",
    )?;
    let client_public_key_bytes: [u8; 32] = client_public_key_bytes
        .try_into()
        .map_err(|_| AssistantCryptoError::InvalidPublicKey)?;
    let client_public_key = PublicKey::from(client_public_key_bytes);

    let encrypt_key =
        derive_directional_key(key.private_key, client_public_key, request_id, b"response");
    let cipher = ChaCha20Poly1305::new_from_slice(&encrypt_key)
        .map_err(|_| AssistantCryptoError::EncryptFailed)?;

    let plaintext = serde_json::to_vec(response)
        .map_err(|err| AssistantCryptoError::InvalidPlaintextPayload(err.to_string()))?;
    let nonce_bytes = build_nonce_bytes();
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce_bytes),
            Payload {
                msg: plaintext.as_slice(),
                aad: request_id.as_bytes(),
            },
        )
        .map_err(|_| AssistantCryptoError::EncryptFailed)?;

    Ok(AssistantEncryptedResponseEnvelope {
        version: ASSISTANT_ENVELOPE_VERSION_V1.to_string(),
        algorithm: ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305.to_string(),
        key_id: key.key_id.clone(),
        request_id: request_id.to_string(),
        nonce: base64::engine::general_purpose::STANDARD.encode(nonce_bytes),
        ciphertext: base64::engine::general_purpose::STANDARD.encode(ciphertext),
    })
}

pub fn derive_public_key_b64(private_key: [u8; 32]) -> String {
    let secret = StaticSecret::from(private_key);
    let public = PublicKey::from(&secret);
    base64::engine::general_purpose::STANDARD.encode(public.as_bytes())
}

fn decode_base64_field(value: &str, field: &'static str) -> Result<Vec<u8>, AssistantCryptoError> {
    base64::engine::general_purpose::STANDARD
        .decode(value.as_bytes())
        .map_err(|_| AssistantCryptoError::InvalidBase64Field { field })
}

fn validate_common_envelope_fields(
    version: &str,
    algorithm: &str,
    request_id: &str,
) -> Result<(), AssistantCryptoError> {
    if version != ASSISTANT_ENVELOPE_VERSION_V1 {
        return Err(AssistantCryptoError::UnsupportedVersion);
    }
    if algorithm != ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305 {
        return Err(AssistantCryptoError::UnsupportedAlgorithm);
    }
    if request_id.trim().is_empty() {
        return Err(AssistantCryptoError::MissingRequestId);
    }
    Ok(())
}

fn derive_directional_key(
    server_private_key_bytes: [u8; 32],
    client_public_key: PublicKey,
    request_id: &str,
    direction: &[u8],
) -> [u8; 32] {
    let secret = StaticSecret::from(server_private_key_bytes);
    let shared_secret = secret.diffie_hellman(&client_public_key);

    let mut hasher = Sha256::new();
    hasher.update(shared_secret.as_bytes());
    hasher.update(b"|");
    hasher.update(request_id.as_bytes());
    hasher.update(b"|");
    hasher.update(direction);
    hasher.finalize().into()
}

fn build_nonce_bytes() -> [u8; 12] {
    let uuid_bytes = uuid::Uuid::new_v4();
    let mut nonce = [0_u8; 12];
    nonce.copy_from_slice(&uuid_bytes.as_bytes()[..12]);
    nonce
}

#[cfg(test)]
mod tests {
    use base64::Engine as _;
    use chacha20poly1305::aead::{Aead, KeyInit, Payload};
    use chacha20poly1305::{ChaCha20Poly1305, Nonce};
    use sha2::Digest;
    use x25519_dalek::{PublicKey, StaticSecret};

    use super::{
        ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305, ASSISTANT_ENVELOPE_VERSION_V1,
        AssistantIngressKeyMaterial, AssistantIngressKeyring, decrypt_assistant_request,
        derive_public_key_b64, encrypt_assistant_response,
    };
    use crate::models::{
        AssistantEncryptedRequestEnvelope, AssistantPlaintextQueryRequest,
        AssistantPlaintextQueryResponse, AssistantQueryCapability, AssistantStructuredPayload,
    };

    #[test]
    fn decrypt_request_and_encrypt_response_round_trip() {
        let server_private_key = [9_u8; 32];
        let client_private_key = StaticSecret::from([5_u8; 32]);
        let request_id = "req-123";
        let request = AssistantPlaintextQueryRequest {
            query: "meetings today".to_string(),
            session_id: Some(uuid::Uuid::new_v4()),
        };
        let request_envelope = encrypt_request_for_test(
            server_private_key,
            &client_private_key,
            request_id,
            &request,
        );

        let keyring = AssistantIngressKeyring {
            active: AssistantIngressKeyMaterial {
                key_id: "assistant-ingress-v1".to_string(),
                private_key: server_private_key,
                public_key: derive_public_key_b64(server_private_key),
                key_expires_at: chrono::Utc::now().timestamp() + 3600,
            },
            previous: None,
        };

        let (decrypted, selected_key) =
            decrypt_assistant_request(&keyring, &request_envelope).expect("decrypt should pass");
        assert_eq!(decrypted.query, request.query);
        assert_eq!(selected_key.key_id, "assistant-ingress-v1");

        let plaintext_response = AssistantPlaintextQueryResponse {
            session_id: uuid::Uuid::new_v4(),
            capability: AssistantQueryCapability::MeetingsToday,
            display_text: "encrypted ingress accepted".to_string(),
            payload: AssistantStructuredPayload {
                title: "Encrypted ingress active".to_string(),
                summary: "encrypted ingress accepted".to_string(),
                key_points: vec!["phase 1 route live".to_string()],
                follow_ups: vec![],
            },
        };
        let response_envelope = encrypt_assistant_response(
            &keyring.active,
            request_id,
            request_envelope.client_ephemeral_public_key.as_str(),
            &plaintext_response,
        )
        .expect("encrypt response should pass");

        let decrypted_response = decrypt_response_for_test(
            &client_private_key,
            request_id,
            response_envelope.nonce.as_str(),
            response_envelope.ciphertext.as_str(),
            server_private_key,
        );
        assert_eq!(
            decrypted_response.display_text,
            "encrypted ingress accepted"
        );
    }

    #[test]
    fn decrypt_rejects_unknown_key_id() {
        let keyring = AssistantIngressKeyring {
            active: AssistantIngressKeyMaterial {
                key_id: "assistant-ingress-v1".to_string(),
                private_key: [9_u8; 32],
                public_key: derive_public_key_b64([9_u8; 32]),
                key_expires_at: chrono::Utc::now().timestamp() + 3600,
            },
            previous: None,
        };

        let envelope = AssistantEncryptedRequestEnvelope {
            version: ASSISTANT_ENVELOPE_VERSION_V1.to_string(),
            algorithm: ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305.to_string(),
            key_id: "missing-key".to_string(),
            request_id: "req-1".to_string(),
            client_ephemeral_public_key: base64::engine::general_purpose::STANDARD
                .encode([1_u8; 32]),
            nonce: base64::engine::general_purpose::STANDARD.encode([1_u8; 12]),
            ciphertext: base64::engine::general_purpose::STANDARD.encode([1_u8; 16]),
        };

        assert!(decrypt_assistant_request(&keyring, &envelope).is_err());
    }

    #[test]
    fn decrypt_rejects_expired_key_id() {
        let server_private_key = [9_u8; 32];
        let client_private_key = StaticSecret::from([5_u8; 32]);
        let mut request_envelope = encrypt_request_for_test(
            [6_u8; 32],
            &client_private_key,
            "req-expired",
            &AssistantPlaintextQueryRequest {
                query: "meetings today".to_string(),
                session_id: None,
            },
        );
        request_envelope.key_id = "assistant-ingress-v0".to_string();

        let keyring = AssistantIngressKeyring {
            active: AssistantIngressKeyMaterial {
                key_id: "assistant-ingress-v1".to_string(),
                private_key: server_private_key,
                public_key: derive_public_key_b64(server_private_key),
                key_expires_at: chrono::Utc::now().timestamp() + 3600,
            },
            previous: Some(AssistantIngressKeyMaterial {
                key_id: "assistant-ingress-v0".to_string(),
                private_key: [6_u8; 32],
                public_key: derive_public_key_b64([6_u8; 32]),
                key_expires_at: chrono::Utc::now().timestamp() - 1,
            }),
        };

        assert!(matches!(
            decrypt_assistant_request(&keyring, &request_envelope),
            Err(super::AssistantCryptoError::ExpiredKeyId)
        ));
    }

    #[test]
    fn decrypt_accepts_active_key_even_when_bootstrap_expiry_has_passed() {
        let server_private_key = [9_u8; 32];
        let client_private_key = StaticSecret::from([5_u8; 32]);
        let request_envelope = encrypt_request_for_test(
            server_private_key,
            &client_private_key,
            "req-active",
            &AssistantPlaintextQueryRequest {
                query: "meetings today".to_string(),
                session_id: None,
            },
        );

        let keyring = AssistantIngressKeyring {
            active: AssistantIngressKeyMaterial {
                key_id: "assistant-ingress-v1".to_string(),
                private_key: server_private_key,
                public_key: derive_public_key_b64(server_private_key),
                key_expires_at: chrono::Utc::now().timestamp() - 1,
            },
            previous: None,
        };

        let result = decrypt_assistant_request(&keyring, &request_envelope);
        assert!(result.is_ok(), "active key should remain usable");
    }

    fn encrypt_request_for_test(
        server_private_key: [u8; 32],
        client_private_key: &StaticSecret,
        request_id: &str,
        request: &AssistantPlaintextQueryRequest,
    ) -> AssistantEncryptedRequestEnvelope {
        let server_public_key = PublicKey::from(&StaticSecret::from(server_private_key));
        let shared_secret = client_private_key.diffie_hellman(&server_public_key);

        let mut hasher = sha2::Sha256::new();
        hasher.update(shared_secret.as_bytes());
        hasher.update(b"|");
        hasher.update(request_id.as_bytes());
        hasher.update(b"|");
        hasher.update(b"request");
        let derived_key: [u8; 32] = hasher.finalize().into();

        let cipher = ChaCha20Poly1305::new_from_slice(&derived_key).expect("cipher should init");
        let nonce = [3_u8; 12];
        let plaintext = serde_json::to_vec(request).expect("request should serialize");
        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: plaintext.as_slice(),
                    aad: request_id.as_bytes(),
                },
            )
            .expect("request encryption should pass");

        AssistantEncryptedRequestEnvelope {
            version: ASSISTANT_ENVELOPE_VERSION_V1.to_string(),
            algorithm: ASSISTANT_ENCRYPTION_ALGORITHM_X25519_CHACHA20POLY1305.to_string(),
            key_id: "assistant-ingress-v1".to_string(),
            request_id: request_id.to_string(),
            client_ephemeral_public_key: base64::engine::general_purpose::STANDARD
                .encode(PublicKey::from(client_private_key).as_bytes()),
            nonce: base64::engine::general_purpose::STANDARD.encode(nonce),
            ciphertext: base64::engine::general_purpose::STANDARD.encode(ciphertext),
        }
    }

    fn decrypt_response_for_test(
        client_private_key: &StaticSecret,
        request_id: &str,
        nonce_b64: &str,
        ciphertext_b64: &str,
        server_private_key: [u8; 32],
    ) -> AssistantPlaintextQueryResponse {
        let server_public_key = PublicKey::from(&StaticSecret::from(server_private_key));
        let shared_secret = client_private_key.diffie_hellman(&server_public_key);

        let mut hasher = sha2::Sha256::new();
        hasher.update(shared_secret.as_bytes());
        hasher.update(b"|");
        hasher.update(request_id.as_bytes());
        hasher.update(b"|");
        hasher.update(b"response");
        let derived_key: [u8; 32] = hasher.finalize().into();

        let cipher = ChaCha20Poly1305::new_from_slice(&derived_key).expect("cipher should init");
        let nonce = base64::engine::general_purpose::STANDARD
            .decode(nonce_b64.as_bytes())
            .expect("nonce should decode");
        let ciphertext = base64::engine::general_purpose::STANDARD
            .decode(ciphertext_b64.as_bytes())
            .expect("ciphertext should decode");
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: ciphertext.as_slice(),
                    aad: request_id.as_bytes(),
                },
            )
            .expect("response decryption should pass");

        serde_json::from_slice(&plaintext).expect("response should parse")
    }
}
