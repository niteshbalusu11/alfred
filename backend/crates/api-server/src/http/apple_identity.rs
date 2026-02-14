use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;

const APPLE_JWKS_URL: &str = "https://appleid.apple.com/auth/keys";
const APPLE_ISSUER: &str = "https://appleid.apple.com";
const MAX_CLOCK_SKEW_SECONDS: i64 = 60;

#[derive(Debug, Clone)]
pub(super) struct VerifiedAppleIdentity {
    pub(super) subject: String,
}

#[derive(Debug, Clone)]
pub(super) enum AppleIdentityError {
    InvalidToken {
        code: &'static str,
        message: &'static str,
    },
    UpstreamUnavailable {
        code: &'static str,
        message: &'static str,
    },
}

#[derive(Debug, Deserialize)]
struct AppleClaims {
    sub: String,
    iat: i64,
}

#[derive(Debug, Deserialize)]
struct AppleJwks {
    keys: Vec<AppleJwk>,
}

#[derive(Debug, Deserialize)]
struct AppleJwk {
    kid: String,
    alg: Option<String>,
    kty: String,
    n: String,
    e: String,
}

pub(super) async fn verify_identity_token(
    http_client: &reqwest::Client,
    expected_audience: &str,
    identity_token: &str,
) -> Result<VerifiedAppleIdentity, AppleIdentityError> {
    if identity_token.trim().is_empty() {
        return Err(AppleIdentityError::InvalidToken {
            code: "invalid_apple_identity_token",
            message: "apple_identity_token is required",
        });
    }

    let header = decode_header(identity_token).map_err(|_| AppleIdentityError::InvalidToken {
        code: "invalid_apple_identity_token",
        message: "Apple identity token is malformed",
    })?;

    if header.alg != Algorithm::RS256 {
        return Err(AppleIdentityError::InvalidToken {
            code: "invalid_apple_identity_token",
            message: "Apple identity token algorithm is unsupported",
        });
    }

    let Some(key_id) = header.kid else {
        return Err(AppleIdentityError::InvalidToken {
            code: "invalid_apple_identity_token",
            message: "Apple identity token key id is missing",
        });
    };

    let jwks: AppleJwks = http_client
        .get(APPLE_JWKS_URL)
        .send()
        .await
        .map_err(|_| AppleIdentityError::UpstreamUnavailable {
            code: "apple_identity_unavailable",
            message: "Unable to reach Apple identity validation endpoint",
        })?
        .error_for_status()
        .map_err(|_| AppleIdentityError::UpstreamUnavailable {
            code: "apple_identity_unavailable",
            message: "Apple identity validation endpoint returned an error",
        })?
        .json()
        .await
        .map_err(|_| AppleIdentityError::UpstreamUnavailable {
            code: "apple_identity_unavailable",
            message: "Apple identity validation response was invalid",
        })?;

    let Some(jwk) = jwks
        .keys
        .iter()
        .find(|key| key.kid == key_id && key.kty == "RSA")
    else {
        return Err(AppleIdentityError::InvalidToken {
            code: "invalid_apple_identity_token",
            message: "Apple identity token key was not recognized",
        });
    };

    if jwk.alg.as_deref().unwrap_or("RS256") != "RS256" {
        return Err(AppleIdentityError::InvalidToken {
            code: "invalid_apple_identity_token",
            message: "Apple identity token key algorithm is unsupported",
        });
    }

    let decoding_key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e).map_err(|_| {
        AppleIdentityError::InvalidToken {
            code: "invalid_apple_identity_token",
            message: "Apple identity token key was invalid",
        }
    })?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[expected_audience]);
    validation.set_issuer(&[APPLE_ISSUER]);
    validation.leeway = MAX_CLOCK_SKEW_SECONDS as u64;
    validation.required_spec_claims = ["exp", "iat", "iss", "aud", "sub"]
        .into_iter()
        .map(str::to_string)
        .collect();

    let token_data =
        decode::<AppleClaims>(identity_token, &decoding_key, &validation).map_err(|err| {
            let (code, message) = match err.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => (
                    "expired_apple_identity_token",
                    "Apple identity token is expired",
                ),
                jsonwebtoken::errors::ErrorKind::InvalidAudience => (
                    "invalid_apple_identity_token",
                    "Apple identity token audience does not match",
                ),
                jsonwebtoken::errors::ErrorKind::InvalidIssuer => (
                    "invalid_apple_identity_token",
                    "Apple identity token issuer is invalid",
                ),
                _ => (
                    "invalid_apple_identity_token",
                    "Apple identity token validation failed",
                ),
            };
            AppleIdentityError::InvalidToken { code, message }
        })?;

    let now = Utc::now().timestamp();
    if token_data.claims.iat > now + MAX_CLOCK_SKEW_SECONDS {
        return Err(AppleIdentityError::InvalidToken {
            code: "invalid_apple_identity_token",
            message: "Apple identity token issue time is invalid",
        });
    }

    if token_data.claims.sub.trim().is_empty() {
        return Err(AppleIdentityError::InvalidToken {
            code: "invalid_apple_identity_token",
            message: "Apple identity token subject is missing",
        });
    }

    Ok(VerifiedAppleIdentity {
        subject: token_data.claims.sub,
    })
}
