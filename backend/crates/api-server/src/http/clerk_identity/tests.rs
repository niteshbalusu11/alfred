use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rand::thread_rng;
use rsa::RsaPrivateKey;
use rsa::pkcs8::EncodePrivateKey;
use rsa::traits::PublicKeyParts;
use serde::Serialize;
use std::sync::OnceLock;

use super::{ClerkIdentityError, ClerkJwk, ClerkJwks, verify_identity_token_with_jwks};

const TEST_KEY_ID: &str = "test-key-id";
const TEST_ISSUER: &str = "https://clerk.example.test";
const TEST_AUDIENCE: &str = "alfred-api";
static TEST_KEY_MATERIAL: OnceLock<TestKeyMaterial> = OnceLock::new();

#[derive(Debug, Serialize)]
struct TestClaims {
    sub: String,
    iat: i64,
    exp: i64,
    iss: String,
    aud: String,
}

struct TestKeyMaterial {
    private_key_pem: String,
    jwk_n: String,
    jwk_e: String,
}

fn test_key_material() -> &'static TestKeyMaterial {
    TEST_KEY_MATERIAL.get_or_init(|| {
        let private_key = RsaPrivateKey::new(&mut thread_rng(), 2048)
            .expect("RSA test key generation should work");
        let public_key = private_key.to_public_key();
        let private_key_pem = private_key
            .to_pkcs8_pem(Default::default())
            .expect("RSA test key serialization should work")
            .to_string();
        let jwk_n = URL_SAFE_NO_PAD.encode(public_key.n().to_bytes_be());
        let jwk_e = URL_SAFE_NO_PAD.encode(public_key.e().to_bytes_be());

        TestKeyMaterial {
            private_key_pem,
            jwk_n,
            jwk_e,
        }
    })
}

#[test]
fn verify_identity_token_accepts_valid_token() {
    let token = signed_token(
        TEST_ISSUER,
        TEST_AUDIENCE,
        Utc::now() + Duration::minutes(5),
    );
    let identity = verify_identity_token_with_jwks(
        &token,
        TEST_ISSUER,
        TEST_AUDIENCE,
        TEST_KEY_ID,
        &test_jwks(),
    )
    .expect("valid token should verify");

    assert_eq!(identity.subject, "clerk_user_123");
}

#[test]
fn verify_identity_token_rejects_expired_token() {
    let token = signed_token(
        TEST_ISSUER,
        TEST_AUDIENCE,
        Utc::now() - Duration::minutes(5),
    );
    let err = verify_identity_token_with_jwks(
        &token,
        TEST_ISSUER,
        TEST_AUDIENCE,
        TEST_KEY_ID,
        &test_jwks(),
    )
    .expect_err("expired token should be rejected");

    assert!(matches!(
        err,
        ClerkIdentityError::InvalidToken {
            code: "expired_clerk_token",
            ..
        }
    ));
}

#[test]
fn verify_identity_token_rejects_invalid_audience() {
    let token = signed_token(
        TEST_ISSUER,
        "other-audience",
        Utc::now() + Duration::minutes(5),
    );
    let err = verify_identity_token_with_jwks(
        &token,
        TEST_ISSUER,
        TEST_AUDIENCE,
        TEST_KEY_ID,
        &test_jwks(),
    )
    .expect_err("wrong audience should be rejected");

    assert!(matches!(
        err,
        ClerkIdentityError::InvalidToken {
            code: "invalid_clerk_token",
            ..
        }
    ));
}

fn signed_token(issuer: &str, audience: &str, expires_at: chrono::DateTime<Utc>) -> String {
    let key_material = test_key_material();
    let now = Utc::now();
    let claims = TestClaims {
        sub: "clerk_user_123".to_string(),
        iat: (now - Duration::minutes(1)).timestamp(),
        exp: expires_at.timestamp(),
        iss: issuer.to_string(),
        aud: audience.to_string(),
    };
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(TEST_KEY_ID.to_string());

    encode(
        &header,
        &claims,
        &EncodingKey::from_rsa_pem(key_material.private_key_pem.as_bytes())
            .expect("private key should parse"),
    )
    .expect("token should encode")
}

fn test_jwks() -> ClerkJwks {
    let key_material = test_key_material();
    ClerkJwks {
        keys: vec![ClerkJwk {
            kid: TEST_KEY_ID.to_string(),
            alg: Some("RS256".to_string()),
            kty: "RSA".to_string(),
            n: key_material.jwk_n.clone(),
            e: key_material.jwk_e.clone(),
            use_: Some("sig".to_string()),
        }],
    }
}
