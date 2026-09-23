use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderMap, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use tokio::sync::Mutex;

pub struct RateLimiter {
    limit: u32,
    window: Duration,
    trusted_proxy_hops: usize,
    clients: Mutex<HashMap<IpAddr, Window>>,
}

struct Window {
    started: Instant,
    requests: u32,
}

impl RateLimiter {
    pub fn per_minute(limit: u32) -> Self {
        Self {
            limit: limit.max(1),
            window: Duration::from_secs(60),
            trusted_proxy_hops: 0,
            clients: Mutex::new(HashMap::new()),
        }
    }

    /// Behind a reverse proxy every connection comes from the proxy, so all
    /// clients would share one bucket. With `hops` proxies in front, the
    /// client is read from `X-Forwarded-For` instead, skipping the entries
    /// those proxies appended. Entries further left are client-supplied and
    /// never trusted.
    pub fn trusting_proxy_hops(mut self, hops: usize) -> Self {
        self.trusted_proxy_hops = hops;
        self
    }

    fn client_ip(&self, headers: &HeaderMap, peer: IpAddr) -> IpAddr {
        if self.trusted_proxy_hops == 0 {
            return peer;
        }
        // The chain as the last proxy saw it: forwarded entries, then the peer.
        let mut chain: Vec<IpAddr> = headers
            .get_all("x-forwarded-for")
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|value| value.split(','))
            .filter_map(|entry| entry.trim().parse().ok())
            .collect();
        chain.push(peer);
        let index = chain.len().saturating_sub(1 + self.trusted_proxy_hops);
        chain[index]
    }

    async fn allow(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let mut clients = self.clients.lock().await;
        if clients.len() > 10_000 {
            clients.retain(|_, entry| now.duration_since(entry.started) < self.window);
        }
        let entry = clients.entry(ip).or_insert(Window {
            started: now,
            requests: 0,
        });
        if now.duration_since(entry.started) >= self.window {
            *entry = Window {
                started: now,
                requests: 0,
            };
        }
        if entry.requests >= self.limit {
            return false;
        }
        entry.requests += 1;
        true
    }
}

pub async fn enforce(
    State(limiter): State<Arc<RateLimiter>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|value| value.0.ip())
        .unwrap_or(IpAddr::from([127, 0, 0, 1]));
    let ip = limiter.client_ip(request.headers(), peer);
    if !limiter.allow(ip).await {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [(axum::http::header::RETRY_AFTER, "60")],
            Json(serde_json::json!({ "error": "rate limit exceeded" })),
        )
            .into_response();
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rejects_requests_above_limit() {
        let limiter = RateLimiter::per_minute(2);
        let ip = IpAddr::from([127, 0, 0, 1]);
        assert!(limiter.allow(ip).await);
        assert!(limiter.allow(ip).await);
        assert!(!limiter.allow(ip).await);
    }

    fn forwarded(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", value.parse().unwrap());
        headers
    }

    #[test]
    fn ignores_forwarded_header_unless_proxies_are_trusted() {
        let limiter = RateLimiter::per_minute(1);
        let peer = IpAddr::from([10, 0, 0, 1]);
        assert_eq!(limiter.client_ip(&forwarded("203.0.113.9"), peer), peer);
    }

    #[test]
    fn reads_the_client_appended_by_the_trusted_proxy() {
        let limiter = RateLimiter::per_minute(1).trusting_proxy_hops(1);
        let peer = IpAddr::from([10, 0, 0, 1]);
        let client = IpAddr::from([203, 0, 113, 9]);
        assert_eq!(limiter.client_ip(&forwarded("203.0.113.9"), peer), client);
        // A spoofed entry prepended by the client is skipped.
        assert_eq!(
            limiter.client_ip(&forwarded("1.2.3.4, 203.0.113.9"), peer),
            client
        );
    }

    #[test]
    fn falls_back_to_the_leftmost_address_when_the_chain_is_short() {
        let limiter = RateLimiter::per_minute(1).trusting_proxy_hops(2);
        let peer = IpAddr::from([10, 0, 0, 1]);
        assert_eq!(limiter.client_ip(&HeaderMap::new(), peer), peer);
    }
}
