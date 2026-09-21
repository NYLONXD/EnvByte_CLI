//! Account endpoints: signing up, signing in, and publishing an identity key.

use serde::{Deserialize, Serialize};

use crate::core::api::client::{http_client, send, send_json, Session};

#[derive(Serialize)]
pub struct SignupRequest<'a> {
    pub username: &'a str,
    pub email: &'a str,
    pub password: &'a str,
}

#[derive(Serialize)]
pub struct LoginRequest<'a> {
    pub email: &'a str,
    pub password: &'a str,
}

#[derive(Deserialize)]
pub struct AuthResponse {
    pub token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    pub user_id: String,
    #[allow(dead_code)]
    pub username: String,
    pub email: String,
}

#[derive(Deserialize)]
pub struct SignupResponse {
    #[serde(default)]
    pub verification_required: bool,
    #[allow(dead_code)]
    #[serde(default)]
    pub email: String,
}

#[derive(Deserialize)]
pub struct Profile {
    #[allow(dead_code)]
    pub id: String,
    pub username: String,
    pub email: String,
    #[serde(default)]
    pub public_key: Option<String>,
}

/// Endpoints below take no session, because the caller does not have one yet.
fn anonymous(server: &str, path: &str) -> Result<reqwest::RequestBuilder, String> {
    Ok(http_client()?.post(format!("{}{path}", server.trim_end_matches('/'))))
}

pub async fn signup(server: &str, request: SignupRequest<'_>) -> Result<SignupResponse, String> {
    send_json(
        anonymous(server, "/auth/signup")?.json(&request),
        "Registration",
    )
    .await
}

pub async fn verify_email(server: &str, email: &str, token: &str) -> Result<AuthResponse, String> {
    send_json(
        anonymous(server, "/auth/verify-email")?
            .json(&serde_json::json!({ "email": email, "token": token })),
        "Email verification",
    )
    .await
}

pub async fn login(server: &str, request: LoginRequest<'_>) -> Result<AuthResponse, String> {
    send_json(anonymous(server, "/auth/login")?.json(&request), "Login").await
}

pub async fn refresh(server: &str, refresh_token: &str) -> Result<AuthResponse, String> {
    send_json(
        anonymous(server, "/auth/refresh")?
            .json(&serde_json::json!({ "refresh_token": refresh_token })),
        "Session refresh",
    )
    .await
}

pub async fn logout(server: &str, refresh_token: &str) -> Result<(), String> {
    send(
        anonymous(server, "/auth/logout")?
            .json(&serde_json::json!({ "refresh_token": refresh_token })),
        "Logout",
    )
    .await
}

pub async fn forgot_password(server: &str, email: &str) -> Result<(), String> {
    send(
        anonymous(server, "/auth/forgot-password")?.json(&serde_json::json!({ "email": email })),
        "Password reset request",
    )
    .await
}

pub async fn reset_password(
    server: &str,
    email: &str,
    token: &str,
    password: &str,
) -> Result<(), String> {
    send(
        anonymous(server, "/auth/reset-password")?.json(&serde_json::json!({
            "email": email,
            "token": token,
            "password": password,
        })),
        "Password reset",
    )
    .await
}

pub async fn me(session: &Session) -> Result<Profile, String> {
    send_json(session.get("/users/me"), "Profile fetch").await
}

/// Publishes this device's identity public key so colleagues can seal project
/// keys to it.
pub async fn publish_identity_key(session: &Session, public_key: &str) -> Result<(), String> {
    send(
        session
            .put("/users/me/key")
            .json(&serde_json::json!({ "public_key": public_key })),
        "Identity key publication",
    )
    .await
}
