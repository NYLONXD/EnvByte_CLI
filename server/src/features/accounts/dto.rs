//! Wire shapes for the account endpoints.
//!
//! Kept apart from the handlers so that a response field is never added by
//! accident while editing logic - these are the API's published contract.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct SignupRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Deserialize)]
pub struct VerifyEmailRequest {
    pub email: String,
    pub token: String,
}

#[derive(Deserialize)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

#[derive(Deserialize)]
pub struct ResetPasswordRequest {
    pub email: String,
    pub token: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct SignupResponse {
    pub verification_required: bool,
    pub email: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub refresh_token: String,
    pub user_id: Uuid,
    pub username: String,
    pub email: String,
}

#[derive(Serialize)]
pub struct MeResponse {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub public_key: Option<String>,
}

#[derive(Deserialize)]
pub struct PublishKeyRequest {
    pub public_key: String,
}
