use sha2::{Digest, Sha256};
use uuid::Uuid;

pub(super) fn hash_token(value: &str) -> Vec<u8> {
    let digest = Sha256::digest(value.as_bytes());
    digest.to_vec()
}

pub(super) fn generate_secure_token(prefix: &str) -> String {
    format!(
        "{prefix}_{}_{}",
        Uuid::new_v4().as_simple(),
        Uuid::new_v4().as_simple()
    )
}
