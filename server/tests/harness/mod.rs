//! Shared scaffolding for the API tests.
//!
//! Every test drives the real router over `oneshot`, so the paths exercised
//! are the ones the CLI actually calls - including middleware and extractors.

#![allow(dead_code)]

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use envbyte_server::{
    app,
    config::{Config, EmailConfig},
    infra::{email::EmailSender, rate_limit::RateLimiter},
    state::AppState,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use tokio::sync::Mutex;
use tower::ServiceExt;

/// Captures what would have been mailed, so tests can read the tokens back.
#[derive(Default)]
pub struct CaptureEmail {
    pub invitation: Mutex<Option<String>>,
    pub verifications: Mutex<HashMap<String, String>>,
    pub resets: Mutex<HashMap<String, String>>,
}

#[async_trait]
impl EmailSender for CaptureEmail {
    async fn send_verification(&self, recipient: &str, token: &str) -> Result<(), String> {
        self.verifications
            .lock()
            .await
            .insert(recipient.to_string(), token.to_string());
        Ok(())
    }

    async fn send_password_reset(&self, recipient: &str, token: &str) -> Result<(), String> {
        self.resets
            .lock()
            .await
            .insert(recipient.to_string(), token.to_string());
        Ok(())
    }

    async fn send_invitation(
        &self,
        _recipient: &str,
        _project: &str,
        token: &str,
        _cli_url: &str,
    ) -> Result<(), String> {
        *self.invitation.lock().await = Some(token.to_string());
        Ok(())
    }
}

pub struct Harness {
    pub router: Router,
    pub email: Arc<CaptureEmail>,
}

impl Harness {
    pub async fn new(pool: PgPool) -> Self {
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let email = Arc::new(CaptureEmail::default());
        let state = AppState {
            pool,
            config: Arc::new(test_config()),
            email: email.clone(),
            general_limiter: Arc::new(RateLimiter::per_minute(10_000)),
            auth_limiter: Arc::new(RateLimiter::per_minute(10_000)),
        };
        Self {
            router: app(state),
            email,
        }
    }

    pub async fn call(
        &self,
        method: &str,
        uri: &str,
        token: Option<&str>,
        body: Value,
    ) -> (StatusCode, Value) {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json");
        if let Some(token) = token {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }
        let bytes = if body.is_null() {
            Vec::new()
        } else {
            serde_json::to_vec(&body).unwrap()
        };
        let response = self
            .router
            .clone()
            .oneshot(builder.body(Body::from(bytes)).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    /// Registers, verifies and signs in an account, then publishes an identity
    /// key for it - the state every project operation assumes.
    pub async fn account(&self, name: &str) -> Account {
        let email = format!("{name}@example.com");
        let (status, _) = self
            .call(
                "POST",
                "/auth/signup",
                None,
                json!({
                    "username": name,
                    "email": email,
                    "password": "a-sufficiently-long-password",
                }),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "signup for {name}");
        let verification = self
            .email
            .verifications
            .lock()
            .await
            .get(&email)
            .cloned()
            .expect("verification token");
        let (status, body) = self
            .call(
                "POST",
                "/auth/verify-email",
                None,
                json!({ "email": email, "token": verification }),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "verify for {name}");
        let token = body["token"].as_str().unwrap().to_string();
        let user_id = body["user_id"].as_str().unwrap().to_string();

        // A distinct, deterministic stand-in for a real X25519 public key. The
        // server only checks the encoding, never the curve point.
        let public_key = BASE64.encode([name.as_bytes()[0]; 32]);
        let (status, _) = self
            .call(
                "PUT",
                "/users/me/key",
                Some(&token),
                json!({ "public_key": public_key }),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "publish key for {name}");

        Account {
            user_id,
            email,
            token,
            public_key,
        }
    }
}

pub struct Account {
    pub user_id: String,
    pub email: String,
    pub token: String,
    pub public_key: String,
}

/// Stands in for a data key sealed to `member`. Opaque to the server, which is
/// the point: it only stores and returns it.
pub fn wrapped_for(member: &str, version: i32) -> String {
    format!("greenbyte:wrap:v1:{member}:{version}")
}

pub fn test_config() -> Config {
    Config {
        environment: "test".to_string(),
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        database_url: "unused".to_string(),
        jwt_secret: "test-secret-that-is-at-least-thirty-two-characters".to_string(),
        jwt_issuer: "envbyte-test".to_string(),
        access_token_ttl_seconds: 3600,
        refresh_token_ttl_days: 30,
        invite_ttl_hours: 24,
        email_verification_ttl_hours: 2,
        password_reset_ttl_minutes: 30,
        database_max_connections: 5,
        public_cli_url: "https://example.com".to_string(),
        rate_limit_per_minute: 10_000,
        auth_rate_limit_per_minute: 10_000,
        trusted_proxy_hops: 0,
        email: EmailConfig::Log,
    }
}
