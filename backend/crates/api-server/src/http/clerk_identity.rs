use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;

const MAX_CLOCK_SKEW_SECONDS: i64 = 60;

#[derive(Debug, Clone)]
pub(super) struct VerifiedClerkIdentity {
    pub(super) subject: String,
}

#[derive(Debug, Clone)]
pub(super) enum ClerkIdentityError {
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
struct ClerkClaims {
    sub: String,
    iat: i64,
}

#[derive(Debug, Deserialize)]
struct ClerkJwks {
    keys: Vec<ClerkJwk>,
}

#[derive(Debug, Deserialize)]
struct ClerkJwk {
    kid: String,
    alg: Option<String>,
    kty: String,
    n: String,
    e: String,
    #[serde(default, rename = "use")]
    use_: Option<String>,
}

pub(super) async fn verify_identity_token(
    http_client: &reqwest::Client,
    jwks_url: &str,
    expected_issuer: &str,
    expected_audience: &str,
    identity_token: &str,
) -> Result<VerifiedClerkIdentity, ClerkIdentityError> {
    if identity_token.trim().is_empty() {
        return Err(ClerkIdentityError::InvalidToken {
            code: "invalid_clerk_token",
            message: "Clerk token is required",
        });
    }

    let header = decode_header(identity_token).map_err(|_| ClerkIdentityError::InvalidToken {
        code: "invalid_clerk_token",
        message: "Clerk token is malformed",
    })?;

    if header.alg != Algorithm::RS256 {
        return Err(ClerkIdentityError::InvalidToken {
            code: "invalid_clerk_token",
            message: "Clerk token algorithm is unsupported",
        });
    }

    let Some(key_id) = header.kid else {
        return Err(ClerkIdentityError::InvalidToken {
            code: "invalid_clerk_token",
            message: "Clerk token key id is missing",
        });
    };

    let jwks: ClerkJwks = http_client
        .get(jwks_url)
        .send()
        .await
        .map_err(|_| ClerkIdentityError::UpstreamUnavailable {
            code: "clerk_jwks_unavailable",
            message: "Unable to reach Clerk JWKS endpoint",
        })?
        .error_for_status()
        .map_err(|_| ClerkIdentityError::UpstreamUnavailable {
            code: "clerk_jwks_unavailable",
            message: "Clerk JWKS endpoint returned an error",
        })?
        .json()
        .await
        .map_err(|_| ClerkIdentityError::UpstreamUnavailable {
            code: "clerk_jwks_unavailable",
            message: "Clerk JWKS response was invalid",
        })?;

    verify_identity_token_with_jwks(
        identity_token,
        expected_issuer,
        expected_audience,
        &key_id,
        &jwks,
    )
}

fn verify_identity_token_with_jwks(
    identity_token: &str,
    expected_issuer: &str,
    expected_audience: &str,
    key_id: &str,
    jwks: &ClerkJwks,
) -> Result<VerifiedClerkIdentity, ClerkIdentityError> {
    let Some(jwk) = jwks.keys.iter().find(|key| {
        key.kid == key_id && key.kty == "RSA" && matches!(key.use_.as_deref(), None | Some("sig"))
    }) else {
        return Err(ClerkIdentityError::InvalidToken {
            code: "invalid_clerk_token",
            message: "Clerk token key was not recognized",
        });
    };

    if jwk.alg.as_deref().unwrap_or("RS256") != "RS256" {
        return Err(ClerkIdentityError::InvalidToken {
            code: "invalid_clerk_token",
            message: "Clerk token key algorithm is unsupported",
        });
    }

    let decoding_key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e).map_err(|_| {
        ClerkIdentityError::InvalidToken {
            code: "invalid_clerk_token",
            message: "Clerk token key was invalid",
        }
    })?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[expected_audience]);
    validation.set_issuer(&[expected_issuer]);
    validation.leeway = MAX_CLOCK_SKEW_SECONDS as u64;
    validation.required_spec_claims = ["exp", "iat", "iss", "aud", "sub"]
        .into_iter()
        .map(str::to_string)
        .collect();

    let token_data =
        decode::<ClerkClaims>(identity_token, &decoding_key, &validation).map_err(|err| {
            let (code, message) = match err.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                    ("expired_clerk_token", "Clerk token is expired")
                }
                jsonwebtoken::errors::ErrorKind::InvalidAudience => {
                    ("invalid_clerk_token", "Clerk token audience does not match")
                }
                jsonwebtoken::errors::ErrorKind::InvalidIssuer => {
                    ("invalid_clerk_token", "Clerk token issuer is invalid")
                }
                _ => ("invalid_clerk_token", "Clerk token validation failed"),
            };
            ClerkIdentityError::InvalidToken { code, message }
        })?;

    let now = Utc::now().timestamp();
    if token_data.claims.iat > now + MAX_CLOCK_SKEW_SECONDS {
        return Err(ClerkIdentityError::InvalidToken {
            code: "invalid_clerk_token",
            message: "Clerk token issue time is invalid",
        });
    }

    let subject = token_data.claims.sub.trim();
    if subject.is_empty() {
        return Err(ClerkIdentityError::InvalidToken {
            code: "invalid_clerk_token",
            message: "Clerk token subject is missing",
        });
    }

    Ok(VerifiedClerkIdentity {
        subject: subject.to_string(),
    })
}

#[cfg(test)]
mod tests;
