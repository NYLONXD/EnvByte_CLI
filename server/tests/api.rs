use std::sync::Arc;

use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use greenbyte_server::{
    app,
    config::{Config, EmailConfig},
    email::EmailSender,
    rate_limit::RateLimiter,
    state::AppState,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use tokio::sync::Mutex;
use tower::ServiceExt;

/// The verifier the project is created with. Pushes must present the same one.
const PROJECT_KEY_HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OTHER_KEY_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[derive(Default)]
struct CaptureEmail {
    token: Mutex<Option<String>>,
    verification_tokens: Mutex<std::collections::HashMap<String, String>>,
}

#[async_trait]
impl EmailSender for CaptureEmail {
    async fn send_verification(&self, recipient: &str, token: &str) -> Result<(), String> {
        self.verification_tokens
            .lock()
            .await
            .insert(recipient.to_string(), token.to_string());
        Ok(())
    }

    async fn send_password_reset(&self, _recipient: &str, _token: &str) -> Result<(), String> {
        Ok(())
    }

    async fn send_invitation(
        &self,
        _recipient: &str,
        _project: &str,
        token: &str,
        _cli_url: &str,
    ) -> Result<(), String> {
        *self.token.lock().await = Some(token.to_string());
        Ok(())
    }
}

#[sqlx::test]
async fn account_project_invitation_and_ciphertext_flow(pool: PgPool) {
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let email = Arc::new(CaptureEmail::default());
    let state = AppState {
        pool,
        config: Arc::new(test_config()),
        email: email.clone(),
        general_limiter: Arc::new(RateLimiter::per_minute(1_000)),
        auth_limiter: Arc::new(RateLimiter::per_minute(1_000)),
    };
    let router = app(state);

    let alice = call_json(
        &router,
        "POST",
        "/auth/signup",
        None,
        json!({"username":"alice","email":"alice@example.com","password":"a-secure-password"}),
    )
    .await;
    assert_eq!(alice.0, StatusCode::OK);
    assert_eq!(alice.1["verification_required"], true);
    let alice_verification = email
        .verification_tokens
        .lock()
        .await
        .get("alice@example.com")
        .cloned()
        .unwrap();
    let alice = call_json(
        &router,
        "POST",
        "/auth/verify-email",
        None,
        json!({"email":"alice@example.com","token":alice_verification}),
    )
    .await;
    assert_eq!(alice.0, StatusCode::OK);
    let alice_token = alice.1["token"].as_str().unwrap();

    let bob = call_json(
        &router,
        "POST",
        "/auth/signup",
        None,
        json!({"username":"bob","email":"bob@example.com","password":"another-secure-password"}),
    )
    .await;
    assert_eq!(bob.0, StatusCode::OK);
    let bob_verification = email
        .verification_tokens
        .lock()
        .await
        .get("bob@example.com")
        .cloned()
        .unwrap();
    let bob = call_json(
        &router,
        "POST",
        "/auth/verify-email",
        None,
        json!({"email":"bob@example.com","token":bob_verification}),
    )
    .await;
    assert_eq!(bob.0, StatusCode::OK);
    let bob_token = bob.1["token"].as_str().unwrap();

    let project = call_json(
        &router,
        "POST",
        "/projects",
        Some(alice_token),
        json!({"name":"payments","master_key_hash":PROJECT_KEY_HASH}),
    )
    .await;
    assert_eq!(project.0, StatusCode::OK);
    let project_id = project.1["project_id"].as_str().unwrap();

    let push_body = json!({
        "project_id":project_id,
        "filename":".env",
        "content":"greenbyte:v2:opaque",
        "message":"initial",
        "commit_id":"11111111-1111-4111-8111-111111111111",
        "master_key_hash":PROJECT_KEY_HASH
    });
    let pushed = call_json(
        &router,
        "POST",
        "/users/me/env",
        Some(alice_token),
        push_body.clone(),
    )
    .await;
    assert_eq!(pushed.0, StatusCode::OK);

    let retried = call_json(
        &router,
        "POST",
        "/users/me/env",
        Some(alice_token),
        push_body,
    )
    .await;
    assert_eq!(retried.0, StatusCode::OK);
    assert_eq!(retried.1["commit_id"], pushed.1["commit_id"]);

    // Content encrypted under a different key must never reach storage.
    let wrong_key = call_json(
        &router,
        "POST",
        "/users/me/env",
        Some(alice_token),
        json!({
            "project_id":project_id,
            "filename":".env",
            "content":"greenbyte:v2:encrypted-with-the-wrong-key",
            "message":"oops",
            "commit_id":"22222222-2222-4222-8222-222222222222",
            "master_key_hash":OTHER_KEY_HASH
        }),
    )
    .await;
    assert_eq!(wrong_key.0, StatusCode::CONFLICT);

    let history = call_json(
        &router,
        "GET",
        &format!("/projects/{project_id}/commits"),
        Some(alice_token),
        Value::Null,
    )
    .await;
    assert_eq!(history.0, StatusCode::OK);
    assert_eq!(history.1.as_array().unwrap().len(), 1);

    let forbidden = call_json(
        &router,
        "GET",
        &format!("/users/me/env?project_id={project_id}"),
        Some(bob_token),
        Value::Null,
    )
    .await;
    assert_eq!(forbidden.0, StatusCode::FORBIDDEN);

    let invited = call_json(
        &router,
        "POST",
        &format!("/projects/{project_id}/collaborators"),
        Some(alice_token),
        json!({"email":"bob@example.com"}),
    )
    .await;
    assert_eq!(invited.0, StatusCode::OK);
    let ott = email.token.lock().await.clone().unwrap();

    // An invitation is not a credential. It cannot be redeemed anonymously,
    // and it cannot be redeemed by whoever it was forwarded to.
    let join_body = json!({
        "ott":ott,
        "refresher_token":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "mac_address":"aabbccddeeff"
    });

    let anonymous = call_json(
        &router,
        "POST",
        "/projects/payments/join",
        None,
        join_body.clone(),
    )
    .await;
    assert_eq!(anonymous.0, StatusCode::UNAUTHORIZED);

    let wrong_account = call_json(
        &router,
        "POST",
        "/projects/payments/join",
        Some(alice_token),
        join_body.clone(),
    )
    .await;
    assert_eq!(wrong_account.0, StatusCode::FORBIDDEN);

    let joined = call_json(
        &router,
        "POST",
        "/projects/payments/join",
        Some(bob_token),
        join_body,
    )
    .await;
    assert_eq!(joined.0, StatusCode::OK);
    // The response must never hand out account credentials.
    assert!(joined.1["auth_token"].is_null());
    assert!(joined.1["refresh_token"].is_null());

    // Bob reaches the project with the session he already had.
    let pulled = call_json(
        &router,
        "GET",
        &format!("/users/me/env?project_id={project_id}"),
        Some(bob_token),
        Value::Null,
    )
    .await;
    assert_eq!(pulled.0, StatusCode::OK);
    assert_eq!(pulled.1[0]["filename"], ".env");
    assert_eq!(pulled.1[0]["content"], "greenbyte:v2:opaque");
}

async fn call_json(
    router: &axum::Router,
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
    let response = router
        .clone()
        .oneshot(builder.body(Body::from(bytes)).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

fn test_config() -> Config {
    Config {
        environment: "test".to_string(),
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        database_url: "unused".to_string(),
        jwt_secret: "test-secret-that-is-at-least-thirty-two-characters".to_string(),
        jwt_issuer: "greenbyte-test".to_string(),
        access_token_ttl_seconds: 3600,
        refresh_token_ttl_days: 30,
        invite_ttl_hours: 24,
        email_verification_ttl_hours: 2,
        password_reset_ttl_minutes: 30,
        database_max_connections: 5,
        public_cli_url: "https://example.com".to_string(),
        rate_limit_per_minute: 1_000,
        auth_rate_limit_per_minute: 1_000,
        email: EmailConfig::Log,
    }
}
