use hmac::{Hmac, Mac};
use sha2::Sha256;

pub const ENCLAVE_RPC_CONTRACT_VERSION_HEADER: &str = "x-alfred-rpc-version";
pub const ENCLAVE_RPC_AUTH_TIMESTAMP_HEADER: &str = "x-alfred-rpc-ts";
pub const ENCLAVE_RPC_AUTH_NONCE_HEADER: &str = "x-alfred-rpc-nonce";
pub const ENCLAVE_RPC_AUTH_SIGNATURE_HEADER: &str = "x-alfred-rpc-signature";

#[derive(Debug, Clone)]
pub struct EnclaveRpcAuthConfig {
    pub shared_secret: String,
    pub max_clock_skew_seconds: u64,
}

pub fn sign_rpc_request(
    shared_secret: &str,
    method: &str,
    path: &str,
    timestamp: i64,
    nonce: &str,
    body: &[u8],
) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(shared_secret.as_bytes())
        .expect("HMAC accepts shared secret key of any size");
    mac.update(method.as_bytes());
    mac.update(&[0u8]);
    mac.update(path.as_bytes());
    mac.update(&[0u8]);
    mac.update(timestamp.to_string().as_bytes());
    mac.update(&[0u8]);
    mac.update(nonce.as_bytes());
    mac.update(&[0u8]);
    mac.update(body);

    let digest = mac.finalize().into_bytes();
    to_lower_hex(digest.as_slice())
}

pub fn constant_time_eq(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }

    let mut diff = 0_u8;
    for (lhs, rhs) in left.as_bytes().iter().zip(right.as_bytes().iter()) {
        diff |= lhs ^ rhs;
    }

    diff == 0
}

fn to_lower_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}
