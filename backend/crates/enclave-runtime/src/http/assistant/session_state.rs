use base64::Engine as _;
use chacha20poly1305::aead::Aead;
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Nonce};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use shared::models::{AssistantQueryCapability, AssistantSessionStateEnvelope};
use shared::repos::AssistantSessionMemory;
use uuid::Uuid;

use crate::RuntimeState;

pub(super) const SESSION_STATE_ALGORITHM: &str = "chacha20poly1305";
pub(super) const SESSION_STATE_VERSION: &str = "v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EnclaveAssistantSessionState {
    pub(super) version: String,
    pub(super) last_capability: AssistantQueryCapability,
    pub(super) memory: AssistantSessionMemory,
}

pub(super) fn decrypt_session_state(
    state: &RuntimeState,
    envelope: &AssistantSessionStateEnvelope,
) -> Result<EnclaveAssistantSessionState, String> {
    let key = state
        .config
        .assistant_ingress_keys
        .key_for_id(envelope.key_id.as_str())
        .ok_or_else(|| "session state key is not recognized".to_string())?;

    if envelope.version != SESSION_STATE_VERSION {
        return Err("session state version is unsupported".to_string());
    }
    if envelope.algorithm != SESSION_STATE_ALGORITHM {
        return Err("session state algorithm is unsupported".to_string());
    }

    let nonce = base64::engine::general_purpose::STANDARD
        .decode(envelope.nonce.as_bytes())
        .map_err(|_| "session state nonce is invalid base64".to_string())?;
    if nonce.len() != 12 {
        return Err("session state nonce must decode to 12 bytes".to_string());
    }

    let ciphertext = base64::engine::general_purpose::STANDARD
        .decode(envelope.ciphertext.as_bytes())
        .map_err(|_| "session state ciphertext is invalid base64".to_string())?;

    let cipher = ChaCha20Poly1305::new((&key.private_key).into());
    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce.as_slice()), ciphertext.as_ref())
        .map_err(|_| "session state decrypt failed".to_string())?;

    serde_json::from_slice::<EnclaveAssistantSessionState>(&plaintext)
        .map_err(|_| "session state payload is invalid".to_string())
}

pub(super) fn encrypt_session_state(
    state: &RuntimeState,
    session_state: &EnclaveAssistantSessionState,
    now: DateTime<Utc>,
) -> Result<AssistantSessionStateEnvelope, String> {
    let key = &state.config.assistant_ingress_keys.active;
    let nonce_source = Uuid::new_v4();
    let nonce_bytes = &nonce_source.as_bytes()[..12];

    let plaintext = serde_json::to_vec(session_state)
        .map_err(|_| "failed to serialize assistant session state".to_string())?;
    let cipher = ChaCha20Poly1305::new((&key.private_key).into());
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(nonce_bytes), plaintext.as_ref())
        .map_err(|_| "failed to encrypt assistant session state".to_string())?;

    Ok(AssistantSessionStateEnvelope {
        version: SESSION_STATE_VERSION.to_string(),
        algorithm: SESSION_STATE_ALGORITHM.to_string(),
        key_id: key.key_id.clone(),
        nonce: base64::engine::general_purpose::STANDARD.encode(nonce_bytes),
        ciphertext: base64::engine::general_purpose::STANDARD.encode(ciphertext),
        expires_at: now + Duration::seconds(state.config.assistant_session_ttl_seconds as i64),
    })
}
