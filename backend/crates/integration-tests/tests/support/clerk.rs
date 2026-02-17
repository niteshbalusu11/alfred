#![allow(dead_code)]

use std::net::SocketAddr;
use std::sync::OnceLock;

use axum::Router;
use axum::routing::get;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rand::thread_rng;
use rsa::RsaPrivateKey;
use rsa::pkcs8::EncodePrivateKey;
use rsa::traits::PublicKeyParts;
use serde::Serialize;

const TEST_KEY_ID: &str = "integration-test-key";
const TEST_ISSUER: &str = "https://clerk.example.test";
const TEST_AUDIENCE: &str = "alfred-api";

static TEST_KEY_MATERIAL: OnceLock<TestKeyMaterial> = OnceLock::new();

pub struct TestClerkAuth {
    pub issuer: String,
    pub audience: String,
    pub jwks_url: String,
    _jwks_server: JwksServer,
}

struct JwksServer {
    _bind_addr: SocketAddr,
    handle: tokio::task::JoinHandle<()>,
}

struct TestKeyMaterial {
    private_key_pem: String,
    jwk_n: String,
    jwk_e: String,
}

#[derive(Debug, Serialize)]
struct TestClaims {
    sub: String,
    iat: i64,
    exp: i64,
    iss: String,
    aud: String,
}

impl TestClerkAuth {
    pub async fn start() -> Self {
        let jwks_json = jwks_json();
        let (bind_addr, handle) = spawn_jwks_server(jwks_json).await;

        Self {
            issuer: TEST_ISSUER.to_string(),
            audience: TEST_AUDIENCE.to_string(),
            jwks_url: format!("http://{bind_addr}/jwks"),
            _jwks_server: JwksServer {
                _bind_addr: bind_addr,
                handle,
            },
        }
    }

    pub fn token_for_subject(&self, subject: &str) -> String {
        signed_token(
            subject,
            &self.issuer,
            &self.audience,
            Utc::now() + Duration::minutes(5),
        )
    }

    pub fn expired_token_for_subject(&self, subject: &str) -> String {
        signed_token(
            subject,
            &self.issuer,
            &self.audience,
            Utc::now() - Duration::minutes(5),
        )
    }

    pub fn token_with_audience(&self, subject: &str, audience: &str) -> String {
        signed_token(
            subject,
            &self.issuer,
            audience,
            Utc::now() + Duration::minutes(5),
        )
    }

    pub fn token_with_issuer(&self, subject: &str, issuer: &str) -> String {
        signed_token(
            subject,
            issuer,
            &self.audience,
            Utc::now() + Duration::minutes(5),
        )
    }
}

impl Drop for JwksServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

async fn spawn_jwks_server(jwks_json: String) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let app = Router::new().route(
        "/jwks",
        get(move || {
            let jwks_json = jwks_json.clone();
            async move {
                (
                    [("cache-control", "public, max-age=300")],
                    axum::Json(
                        serde_json::from_str::<serde_json::Value>(&jwks_json)
                            .expect("jwks json should be valid"),
                    ),
                )
            }
        }),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("jwks listener should bind");
    let bind_addr = listener
        .local_addr()
        .expect("jwks listener local address should be available");

    let handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("jwks server should run");
    });

    (bind_addr, handle)
}

fn signed_token(
    subject: &str,
    issuer: &str,
    audience: &str,
    expires_at: chrono::DateTime<Utc>,
) -> String {
    let key_material = test_key_material();
    let now = Utc::now();
    let claims = TestClaims {
        sub: subject.to_string(),
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

fn jwks_json() -> String {
    let key_material = test_key_material();
    serde_json::json!({
        "keys": [{
            "kid": TEST_KEY_ID,
            "alg": "RS256",
            "kty": "RSA",
            "n": key_material.jwk_n,
            "e": key_material.jwk_e,
            "use": "sig"
        }]
    })
    .to_string()
}

fn test_key_material() -> &'static TestKeyMaterial {
    TEST_KEY_MATERIAL.get_or_init(|| {
        let private_key =
            RsaPrivateKey::new(&mut thread_rng(), 2048).expect("RSA key generation should work");
        let public_key = private_key.to_public_key();

        let private_key_pem = private_key
            .to_pkcs8_pem(Default::default())
            .expect("RSA private key serialization should work")
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
