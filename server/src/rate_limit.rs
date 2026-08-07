use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    extract::{ConnectInfo, State},
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use tokio::sync::Mutex;

pub struct RateLimiter {
    limit: u32,
    window: Duration,
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
            clients: Mutex::new(HashMap::new()),
        }
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
    let ip = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|value| value.0.ip())
        .unwrap_or(IpAddr::from([127, 0, 0, 1]));
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
}
