use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde::Serialize;

use super::{ClerkIdentityError, ClerkJwk, ClerkJwks, verify_identity_token_with_jwks};

const TEST_KEY_ID: &str = "test-key-id";
const TEST_ISSUER: &str = "https://clerk.example.test";
const TEST_AUDIENCE: &str = "alfred-api";
const TEST_PRIVATE_KEY_PEM: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQCwIawADu0yuUNb
ot21aOFThwlTS3ixKy4uP2wj5zf5B9ASYtj21neyPTx/RLaH7+u9AtDXJnojJe4C
xA9nw/3l2YNhgrcVWJfQJ8nieGxuSvqne+gheUV63pIjrvOHugw4A8snMDEg1iwx
DPxaILUulnvQKr7JybldWfcpessK3GDJG9oLhm16L9mgnjdkPIBEaNvEVCCi2sqP
C9gkoGmbcFWNURGOlLRsKTARlaRbk5gmxc3fSVy4OA+pKPGiTO9LBocZy10cPxxz
QuwCxtPvNELZYBbQygGURg+FfZ3+jMjMluf/6ry2Ul0vZZhQiCLzjwlDeOQNI4tH
E1onc+FJAgMBAAECggEAFR6sg8NGcQ8jAxF3+WFOp5fpJ9pEaiYt3vDO9E0s+okT
y6ibsJmN98r8/pvMMWe1AlCcnzfnbGCHdkBxQnWPc+jNprsoXgBmD9k9jZD4G4+V
F9E1SBJFIRMgPfQkEpHeFjdqPFQ8h932pZkTh3ElmKUFyrPddc/hEM8RYqFtSGtq
noRUEsg5qqN3bA4eAjfU4gsL6wKQzk5V6d0+kOVPtFHT9I7vlTd4o/9fFLVA/bgN
nRDLHsSlD2ddZCLS/WMbMD2vywb9soryQvz7d9GVlpkW2qCda5/XVcjXQn9kSHd3
eg1H5Haft6AWW9eUgRMh9014qBxwIYFcgls95QdOAQKBgQDc/02cSiCKg/zxfp3i
WEpzWJ4YrYkBA26PBQngtW/0Ln9URXC4ltTj99gUmPH7BVBI61afEHeQXwoXv+yM
JUjeUiMdyfLd7Koswjld3Lg03ag95ss7IVPv0/lA1ChacQ6W8ptXv4exTCksmz3n
AYKqkZ5IsjxtIzXPZ3q/+anZgQKBgQDMBzalHbm5GxId6lf1yG/COloQ5Tqz9m3e
2nMOm5QOoIqC57cJTDezBdWTagjFf/3YonnQG11s5gZxocoQ8jz5dQpOZaFBrlCU
3wsSTOs2zXDNsPlaqNsjZ0N5Dh8UnXJ4GfhfccWZFJbwgO/lBLG/g8oFPDm8jBfa
u3X/h+ebyQKBgQCtledLLLp0som60n6HLFyGT4QW1C/52M09j3Kby0f9n4wqEEUi
6G6eBa33N89SIXFXZWrrlA6mGtCdqQXPavXakt+8ZUTb5iog8AoJXPZfp/+fZ6oY
buw0Q+bTchGkQIt6K2OzP+EAdVceD25HBduxyKFwbneiLfb1S02SfzNXgQKBgBJs
fFTAsGq0tRgad0LsjJr/Ze6spHZnxFghZc5l4iLIAHn9rpuaVFVIK5caNhyPiD6t
vU47il4xD1fngjWxiiwEk5+ssbkaopAu6/MFGyBhwNPyLTIwmUlDI+akjc3wwcty
nOkRfwRpxY+GNSN7HwnqPq3mWFhcVjMcRnWCsjlxAoGAVgu4Ga0Sdt/g/X+oTPIn
LKaT19xSNMvK9v3j0sjBFQd6BPH/rXZX9qS/FViponuDDgDZzM5a0Q/2TyMHmmIz
YAy2DrR10E8FmqgknK4HgTKo5QMPnRF5j0cM3InTiIP/tiEasZxXJ2Tzbjj8T2hC
6daSmjGDlaSF9Aq1hA/0CT4=
-----END PRIVATE KEY-----"#;
const TEST_JWK_N: &str = "sCGsAA7tMrlDW6LdtWjhU4cJU0t4sSsuLj9sI-c3-QfQEmLY9tZ3sj08f0S2h-_rvQLQ1yZ6IyXuAsQPZ8P95dmDYYK3FViX0CfJ4nhsbkr6p3voIXlFet6SI67zh7oMOAPLJzAxINYsMQz8WiC1LpZ70Cq-ycm5XVn3KXrLCtxgyRvaC4Ztei_ZoJ43ZDyARGjbxFQgotrKjwvYJKBpm3BVjVERjpS0bCkwEZWkW5OYJsXN30lcuDgPqSjxokzvSwaHGctdHD8cc0LsAsbT7zRC2WAW0MoBlEYPhX2d_ozIzJbn_-q8tlJdL2WYUIgi848JQ3jkDSOLRxNaJ3PhSQ";
const TEST_JWK_E: &str = "AQAB";

#[derive(Debug, Serialize)]
struct TestClaims {
    sub: String,
    iat: i64,
    exp: i64,
    iss: String,
    aud: String,
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
        &EncodingKey::from_rsa_pem(TEST_PRIVATE_KEY_PEM.as_bytes())
            .expect("private key should parse"),
    )
    .expect("token should encode")
}

fn test_jwks() -> ClerkJwks {
    ClerkJwks {
        keys: vec![ClerkJwk {
            kid: TEST_KEY_ID.to_string(),
            alg: Some("RS256".to_string()),
            kty: "RSA".to_string(),
            n: TEST_JWK_N.to_string(),
            e: TEST_JWK_E.to_string(),
            use_: Some("sig".to_string()),
        }],
    }
}
