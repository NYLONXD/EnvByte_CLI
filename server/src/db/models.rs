use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub public_key: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct Invitation {
    pub id: Uuid,
    pub project_id: Uuid,
    pub user_id: Uuid,
    pub email: String,
    pub expires_at: DateTime<Utc>,
    pub consumed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct EnvFileResponse {
    pub filename: String,
    #[sqlx(rename = "encrypted_content")]
    pub content: String,
    pub key_version: i32,
}

/// A project data key sealed to one member's identity public key.
#[derive(Debug, Serialize, FromRow)]
pub struct KeyGrant {
    pub key_version: i32,
    pub wrapped_key: String,
}
