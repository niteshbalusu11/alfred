use std::sync::Arc;

use redis::AsyncCommands;
use redis::aio::ConnectionManager;
use reqwest::header::{CACHE_CONTROL, HeaderMap};
use serde::{Deserialize, Serialize};
use tracing::warn;

const MIN_CACHE_CONTROL_TTL_SECONDS: u64 = 60;
const MAX_CACHE_CONTROL_TTL_SECONDS: u64 = 3600;

#[derive(Debug, Clone)]
pub struct ClerkJwksCacheConfig {
    pub redis_url: String,
    pub cache_key: String,
    pub default_ttl_seconds: u64,
    pub stale_ttl_seconds: u64,
}

#[derive(Clone)]
pub struct ClerkJwksCache {
    connection: ConnectionManager,
    config: ClerkJwksCacheConfig,
    refresh_lock: Arc<tokio::sync::Mutex<()>>,
}

#[derive(Debug)]
pub enum ClerkJwksCacheError {
    UnknownKeyId,
    UpstreamUnavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedJwksEntry {
    jwks_json: String,
    fetched_at: i64,
    expires_at: i64,
    stale_until: i64,
}

#[derive(Debug, Deserialize)]
struct JwksEnvelope {
    #[serde(default)]
    keys: Vec<JwksKey>,
}

#[derive(Debug, Deserialize)]
struct JwksKey {
    kid: String,
}

impl ClerkJwksCache {
    pub async fn new(config: ClerkJwksCacheConfig) -> Result<Self, String> {
        if config.default_ttl_seconds == 0 {
            return Err("clerk jwks cache default ttl must be greater than 0".to_string());
        }
        if config.stale_ttl_seconds == 0 {
            return Err("clerk jwks cache stale ttl must be greater than 0".to_string());
        }
        if config.cache_key.trim().is_empty() {
            return Err("clerk jwks cache key must not be empty".to_string());
        }

        let client =
            redis::Client::open(config.redis_url.as_str()).map_err(|err| err.to_string())?;
        let connection = ConnectionManager::new(client)
            .await
            .map_err(|err| err.to_string())?;

        let mut health_connection = connection.clone();
        redis::cmd("PING")
            .query_async::<String>(&mut health_connection)
            .await
            .map_err(|err| format!("failed to connect to redis: {err}"))?;

        Ok(Self {
            connection,
            config,
            refresh_lock: Arc::new(tokio::sync::Mutex::new(())),
        })
    }

    pub async fn load_jwks_for_key(
        &self,
        http_client: &reqwest::Client,
        jwks_url: &str,
        key_id: &str,
    ) -> Result<String, ClerkJwksCacheError> {
        let now = unix_timestamp();
        if let Some(cached) = self.read_cached_entry().await
            && now <= cached.expires_at
            && jwks_contains_key(&cached.jwks_json, key_id)
        {
            return Ok(cached.jwks_json);
        }

        let _refresh_guard = self.refresh_lock.lock().await;

        let now = unix_timestamp();
        let cached_after_lock = self.read_cached_entry().await;
        if let Some(cached) = cached_after_lock.as_ref()
            && now <= cached.expires_at
            && jwks_contains_key(&cached.jwks_json, key_id)
        {
            return Ok(cached.jwks_json.clone());
        }

        let stale_fallback = cached_after_lock.as_ref().and_then(|cached| {
            (now <= cached.stale_until && jwks_contains_key(&cached.jwks_json, key_id))
                .then(|| cached.jwks_json.clone())
        });

        match self.fetch_and_cache_jwks(http_client, jwks_url).await {
            Ok(fetched) => {
                if jwks_contains_key(&fetched.jwks_json, key_id) {
                    Ok(fetched.jwks_json)
                } else {
                    Err(ClerkJwksCacheError::UnknownKeyId)
                }
            }
            Err(err) => {
                if let Some(stale_jwks) = stale_fallback {
                    warn!(
                        key_id = %key_id,
                        "using stale Clerk JWKS cache entry because refresh failed"
                    );
                    Ok(stale_jwks)
                } else {
                    Err(err)
                }
            }
        }
    }

    async fn read_cached_entry(&self) -> Option<CachedJwksEntry> {
        let mut connection = self.connection.clone();
        let raw: Option<String> = match connection.get(&self.config.cache_key).await {
            Ok(raw) => raw,
            Err(err) => {
                warn!("failed to read Clerk JWKS cache entry from redis: {err}");
                return None;
            }
        };

        raw.and_then(
            |payload| match serde_json::from_str::<CachedJwksEntry>(&payload) {
                Ok(parsed) => Some(parsed),
                Err(err) => {
                    warn!("failed to parse Clerk JWKS cache entry from redis: {err}");
                    None
                }
            },
        )
    }

    async fn fetch_and_cache_jwks(
        &self,
        http_client: &reqwest::Client,
        jwks_url: &str,
    ) -> Result<CachedJwksEntry, ClerkJwksCacheError> {
        let response = http_client
            .get(jwks_url)
            .send()
            .await
            .map_err(|_| ClerkJwksCacheError::UpstreamUnavailable)?;
        if !response.status().is_success() {
            return Err(ClerkJwksCacheError::UpstreamUnavailable);
        }

        let ttl_seconds =
            resolve_cache_ttl_seconds(response.headers(), self.config.default_ttl_seconds);
        let body = response
            .text()
            .await
            .map_err(|_| ClerkJwksCacheError::UpstreamUnavailable)?;

        if !looks_like_jwks(&body) {
            return Err(ClerkJwksCacheError::UpstreamUnavailable);
        }

        let now = unix_timestamp();
        let expires_at = now.saturating_add(i64::try_from(ttl_seconds).unwrap_or(i64::MAX));
        let stale_until = expires_at
            .saturating_add(i64::try_from(self.config.stale_ttl_seconds).unwrap_or(i64::MAX));

        let entry = CachedJwksEntry {
            jwks_json: body,
            fetched_at: now,
            expires_at,
            stale_until,
        };

        let redis_ttl_seconds = ttl_seconds.saturating_add(self.config.stale_ttl_seconds);
        self.write_cached_entry(&entry, redis_ttl_seconds).await;

        Ok(entry)
    }

    async fn write_cached_entry(&self, entry: &CachedJwksEntry, ttl_seconds: u64) {
        let serialized = match serde_json::to_string(entry) {
            Ok(serialized) => serialized,
            Err(err) => {
                warn!("failed to serialize Clerk JWKS cache entry: {err}");
                return;
            }
        };

        let mut connection = self.connection.clone();
        if let Err(err) = connection
            .set_ex::<_, _, ()>(&self.config.cache_key, serialized, ttl_seconds)
            .await
        {
            warn!("failed to write Clerk JWKS cache entry to redis: {err}");
        }
    }
}

fn looks_like_jwks(jwks_json: &str) -> bool {
    serde_json::from_str::<JwksEnvelope>(jwks_json)
        .map(|jwks| !jwks.keys.is_empty())
        .unwrap_or(false)
}

fn jwks_contains_key(jwks_json: &str, key_id: &str) -> bool {
    serde_json::from_str::<JwksEnvelope>(jwks_json)
        .map(|jwks| jwks.keys.into_iter().any(|key| key.kid == key_id))
        .unwrap_or(false)
}

fn resolve_cache_ttl_seconds(headers: &HeaderMap, default_ttl_seconds: u64) -> u64 {
    let ttl_seconds = headers
        .get(CACHE_CONTROL)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_cache_control_max_age)
        .unwrap_or(default_ttl_seconds);

    ttl_seconds.clamp(MIN_CACHE_CONTROL_TTL_SECONDS, MAX_CACHE_CONTROL_TTL_SECONDS)
}

fn parse_cache_control_max_age(cache_control: &str) -> Option<u64> {
    for directive in cache_control.split(',') {
        let normalized = directive.trim().to_ascii_lowercase();
        if let Some(value) = normalized.strip_prefix("max-age=")
            && let Ok(parsed) = value.parse::<u64>()
        {
            return Some(parsed);
        }
    }

    None
}

fn unix_timestamp() -> i64 {
    chrono::Utc::now().timestamp()
}

#[cfg(test)]
mod tests {
    use reqwest::header::{HeaderMap, HeaderValue};

    use super::{jwks_contains_key, parse_cache_control_max_age, resolve_cache_ttl_seconds};

    #[test]
    fn parse_cache_control_max_age_reads_valid_directive() {
        assert_eq!(
            parse_cache_control_max_age("public, max-age=120, must-revalidate"),
            Some(120)
        );
    }

    #[test]
    fn parse_cache_control_max_age_ignores_invalid_values() {
        assert_eq!(parse_cache_control_max_age("private, max-age=abc"), None);
        assert_eq!(parse_cache_control_max_age("no-store"), None);
    }

    #[test]
    fn resolve_cache_ttl_seconds_clamps_to_safe_range() {
        let mut headers = HeaderMap::new();
        headers.insert("cache-control", HeaderValue::from_static("max-age=5"));
        assert_eq!(resolve_cache_ttl_seconds(&headers, 300), 60);

        headers.insert("cache-control", HeaderValue::from_static("max-age=7200"));
        assert_eq!(resolve_cache_ttl_seconds(&headers, 300), 3600);
    }

    #[test]
    fn jwks_contains_key_checks_known_kid() {
        let jwks = r#"{"keys":[{"kid":"kid-a"},{"kid":"kid-b"}]}"#;
        assert!(jwks_contains_key(jwks, "kid-a"));
        assert!(!jwks_contains_key(jwks, "kid-missing"));
    }
}
