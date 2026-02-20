use std::collections::{HashMap, HashSet, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, Request, State};
use axum::http::Method;
use axum::middleware::Next;
use axum::response::Response;
use tracing::warn;

use super::errors::too_many_requests_response;
use super::{AppState, AuthUser};

#[derive(Clone, Default)]
pub struct RateLimiter {
    entries: Arc<Mutex<HashMap<RateLimitBucketKey, VecDeque<Instant>>>>,
}

#[derive(Debug, Clone, Copy)]
enum SensitiveEndpoint {
    GoogleConnectStart,
    GoogleConnectCallback,
    RevokeConnector,
    PrivacyDeleteAll,
    AutomationCreate,
    AutomationUpdate,
    AutomationDelete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RateLimitDecision {
    Allowed,
    Denied { retry_after_seconds: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RateLimitBucketKey {
    endpoint: &'static str,
    subject: String,
}

#[derive(Debug, Clone, Copy)]
struct RateLimitPolicy {
    max_requests: usize,
    window_seconds: u64,
}

const MAX_TRACKED_WINDOW_SECONDS: u64 = 3600;

impl SensitiveEndpoint {
    fn from_request(req: &Request) -> Option<Self> {
        let method = req.method();
        let path = req.uri().path();

        match (method, path) {
            (&Method::POST, "/v1/connectors/google/start") => Some(Self::GoogleConnectStart),
            (&Method::POST, "/v1/connectors/google/callback") => Some(Self::GoogleConnectCallback),
            (&Method::DELETE, path) if path.starts_with("/v1/connectors/") => {
                Some(Self::RevokeConnector)
            }
            (&Method::POST, "/v1/privacy/delete-all") => Some(Self::PrivacyDeleteAll),
            (&Method::POST, "/v1/automations") => Some(Self::AutomationCreate),
            (&Method::PATCH, path) if path.starts_with("/v1/automations/") => {
                Some(Self::AutomationUpdate)
            }
            (&Method::DELETE, path) if path.starts_with("/v1/automations/") => {
                Some(Self::AutomationDelete)
            }
            _ => None,
        }
    }

    fn key_name(self) -> &'static str {
        match self {
            Self::GoogleConnectStart => "google_connect_start",
            Self::GoogleConnectCallback => "google_connect_callback",
            Self::RevokeConnector => "revoke_connector",
            Self::PrivacyDeleteAll => "privacy_delete_all",
            Self::AutomationCreate => "automation_create",
            Self::AutomationUpdate => "automation_update",
            Self::AutomationDelete => "automation_delete",
        }
    }

    fn policy(self) -> RateLimitPolicy {
        match self {
            Self::GoogleConnectStart => RateLimitPolicy {
                max_requests: 20,
                window_seconds: 60,
            },
            Self::GoogleConnectCallback => RateLimitPolicy {
                max_requests: 20,
                window_seconds: 60,
            },
            Self::RevokeConnector => RateLimitPolicy {
                max_requests: 10,
                window_seconds: 60,
            },
            Self::PrivacyDeleteAll => RateLimitPolicy {
                max_requests: 3,
                window_seconds: 3600,
            },
            Self::AutomationCreate => RateLimitPolicy {
                max_requests: 20,
                window_seconds: 60,
            },
            Self::AutomationUpdate => RateLimitPolicy {
                max_requests: 30,
                window_seconds: 60,
            },
            Self::AutomationDelete => RateLimitPolicy {
                max_requests: 20,
                window_seconds: 60,
            },
        }
    }
}

impl RateLimiter {
    pub fn spawn_pruner(&self, interval: Duration) -> tokio::task::JoinHandle<()> {
        let entries = Arc::clone(&self.entries);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                prune_entries(&entries, Instant::now());
            }
        })
    }

    fn check(&self, endpoint: SensitiveEndpoint, subject: &str) -> RateLimitDecision {
        self.check_at(endpoint, subject, Instant::now())
    }

    fn check_at(
        &self,
        endpoint: SensitiveEndpoint,
        subject: &str,
        now: Instant,
    ) -> RateLimitDecision {
        let policy = endpoint.policy();
        let window = Duration::from_secs(policy.window_seconds);
        let cutoff = now.checked_sub(window).unwrap_or(now);
        let bucket_key = RateLimitBucketKey {
            endpoint: endpoint.key_name(),
            subject: subject.to_string(),
        };

        let mut entries = self
            .entries
            .lock()
            .expect("rate limiter mutex should not be poisoned");

        let bucket = entries.entry(bucket_key).or_default();
        prune_bucket(bucket, cutoff);

        if bucket.len() >= policy.max_requests {
            let retry_after_seconds = bucket
                .front()
                .map(|first_seen| {
                    let elapsed = now.saturating_duration_since(*first_seen);
                    window.saturating_sub(elapsed).as_secs().max(1)
                })
                .unwrap_or(policy.window_seconds);
            return RateLimitDecision::Denied {
                retry_after_seconds,
            };
        }

        bucket.push_back(now);

        RateLimitDecision::Allowed
    }
}

fn prune_entries(
    entries: &Arc<Mutex<HashMap<RateLimitBucketKey, VecDeque<Instant>>>>,
    now: Instant,
) {
    let global_cutoff = now
        .checked_sub(Duration::from_secs(MAX_TRACKED_WINDOW_SECONDS))
        .unwrap_or(now);
    let mut state = entries
        .lock()
        .expect("rate limiter prune mutex should not be poisoned");

    state.retain(|_, bucket| {
        prune_bucket(bucket, global_cutoff);
        !bucket.is_empty()
    });
}

fn prune_bucket(bucket: &mut VecDeque<Instant>, cutoff: Instant) {
    while let Some(front) = bucket.front() {
        if *front <= cutoff {
            bucket.pop_front();
        } else {
            break;
        }
    }
}

pub(super) async fn sensitive_rate_limit_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let Some(endpoint) = SensitiveEndpoint::from_request(&req) else {
        return next.run(req).await;
    };

    let subject = request_subject(&req, &state.trusted_proxy_ips);

    match state.rate_limiter.check(endpoint, &subject) {
        RateLimitDecision::Allowed => next.run(req).await,
        RateLimitDecision::Denied {
            retry_after_seconds,
        } => {
            warn!(
                endpoint = endpoint.key_name(),
                retry_after_seconds, "request denied by endpoint rate limit",
            );
            too_many_requests_response(retry_after_seconds)
        }
    }
}

fn request_subject(req: &Request, trusted_proxy_ips: &HashSet<IpAddr>) -> String {
    if let Some(user) = req.extensions().get::<AuthUser>() {
        return format!("user:{}", user.user_id);
    }

    if let Some(ip) = remote_ip(req, trusted_proxy_ips) {
        return format!("ip:{ip}");
    }

    "anonymous".to_string()
}

fn remote_ip(req: &Request, trusted_proxy_ips: &HashSet<IpAddr>) -> Option<IpAddr> {
    let peer_ip = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|connect_info| connect_info.0.ip())?;

    if !trusted_proxy_ips.contains(&peer_ip) {
        return Some(peer_ip);
    }

    forwarded_client_ip(req, trusted_proxy_ips, peer_ip).or(Some(peer_ip))
}

fn forwarded_client_ip(
    req: &Request,
    trusted_proxy_ips: &HashSet<IpAddr>,
    peer_ip: IpAddr,
) -> Option<IpAddr> {
    let mut chain = forwarded_for_chain(req);
    if !chain.is_empty() {
        chain.push(peer_ip);
        if let Some(client_ip) = first_untrusted_from_right(&chain, trusted_proxy_ips) {
            return Some(client_ip);
        }
    }

    req.headers()
        .get("x-real-ip")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<IpAddr>().ok())
}

fn forwarded_for_chain(req: &Request) -> Vec<IpAddr> {
    req.headers()
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(parse_ip_chain)
        .collect()
}

fn parse_ip_chain(raw: &str) -> Vec<IpAddr> {
    raw.split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .filter_map(|entry| entry.parse::<IpAddr>().ok())
        .collect()
}

fn first_untrusted_from_right(
    chain: &[IpAddr],
    trusted_proxy_ips: &HashSet<IpAddr>,
) -> Option<IpAddr> {
    chain
        .iter()
        .rev()
        .find(|ip| !trusted_proxy_ips.contains(ip))
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::header::HeaderName;
    use std::collections::HashSet;

    #[test]
    fn allows_until_limit_then_denies() {
        let limiter = RateLimiter::default();
        let start = Instant::now();

        for _ in 0..20 {
            assert_eq!(
                limiter.check_at(SensitiveEndpoint::GoogleConnectStart, "ip:1.2.3.4", start),
                RateLimitDecision::Allowed
            );
        }

        let denied = limiter.check_at(SensitiveEndpoint::GoogleConnectStart, "ip:1.2.3.4", start);
        assert!(matches!(
            denied,
            RateLimitDecision::Denied {
                retry_after_seconds: 1..=60
            }
        ));
    }

    #[test]
    fn different_endpoints_have_independent_limits() {
        let limiter = RateLimiter::default();
        let start = Instant::now();

        for _ in 0..20 {
            assert_eq!(
                limiter.check_at(SensitiveEndpoint::GoogleConnectStart, "ip:1.2.3.4", start),
                RateLimitDecision::Allowed
            );
        }

        assert_eq!(
            limiter.check_at(
                SensitiveEndpoint::GoogleConnectCallback,
                "ip:1.2.3.4",
                start
            ),
            RateLimitDecision::Allowed
        );
    }

    #[test]
    fn window_resets_after_expiration() {
        let limiter = RateLimiter::default();
        let start = Instant::now();
        let after_window = start + Duration::from_secs(61);

        for _ in 0..20 {
            assert_eq!(
                limiter.check_at(SensitiveEndpoint::GoogleConnectStart, "ip:1.2.3.4", start),
                RateLimitDecision::Allowed
            );
        }

        assert_eq!(
            limiter.check_at(
                SensitiveEndpoint::GoogleConnectStart,
                "ip:1.2.3.4",
                after_window
            ),
            RateLimitDecision::Allowed
        );
    }

    #[test]
    fn stale_buckets_are_pruned() {
        let limiter = RateLimiter::default();
        let start = Instant::now();
        let stale_cutoff = start + Duration::from_secs(MAX_TRACKED_WINDOW_SECONDS + 1);

        assert_eq!(
            limiter.check_at(SensitiveEndpoint::GoogleConnectStart, "user:stale", start),
            RateLimitDecision::Allowed
        );
        prune_entries(&limiter.entries, stale_cutoff);

        let entries = limiter
            .entries
            .lock()
            .expect("test mutex should not be poisoned");
        assert!(entries.is_empty());
    }

    #[test]
    fn request_subject_prefers_connect_info_over_spoofable_forward_headers() {
        let trusted_proxy_ips = HashSet::new();
        let mut request = Request::builder()
            .uri("/v1/connectors/google/start")
            .body(Body::empty())
            .expect("request builder should work");

        request.headers_mut().insert(
            HeaderName::from_static("x-forwarded-for"),
            "203.0.113.99".parse().expect("header value should parse"),
        );
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([10, 20, 30, 40], 8080))));

        let subject = request_subject(&request, &trusted_proxy_ips);
        assert_eq!(subject, "ip:10.20.30.40");
    }

    #[test]
    fn request_subject_uses_forwarded_chain_when_peer_is_trusted_proxy() {
        let trusted_proxy_ips = HashSet::from([IpAddr::from([10, 0, 0, 5])]);
        let mut request = Request::builder()
            .uri("/v1/connectors/google/start")
            .body(Body::empty())
            .expect("request builder should work");

        request.headers_mut().insert(
            HeaderName::from_static("x-forwarded-for"),
            "198.51.100.20, 10.0.0.5"
                .parse()
                .expect("header value should parse"),
        );
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([10, 0, 0, 5], 8080))));

        let subject = request_subject(&request, &trusted_proxy_ips);
        assert_eq!(subject, "ip:198.51.100.20");
    }

    #[test]
    fn request_subject_consumes_all_xff_header_values() {
        let trusted_proxy_ips =
            HashSet::from([IpAddr::from([10, 0, 0, 5]), IpAddr::from([10, 0, 0, 9])]);
        let mut request = Request::builder()
            .uri("/v1/connectors/google/start")
            .body(Body::empty())
            .expect("request builder should work");

        request.headers_mut().append(
            HeaderName::from_static("x-forwarded-for"),
            "203.0.113.250".parse().expect("header value should parse"),
        );
        request.headers_mut().append(
            HeaderName::from_static("x-forwarded-for"),
            "198.51.100.20, 10.0.0.9"
                .parse()
                .expect("header value should parse"),
        );
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([10, 0, 0, 5], 8080))));

        let subject = request_subject(&request, &trusted_proxy_ips);
        assert_eq!(subject, "ip:198.51.100.20");
    }
}
