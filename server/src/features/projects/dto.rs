//! Wire shapes for the project endpoints.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    /// The project data key sealed to the creator's own identity key.
    pub wrapped_key: String,
}

#[derive(Serialize)]
pub struct CreateProjectResponse {
    pub project_id: Uuid,
    pub key_version: i32,
    pub soft_token: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ProjectSummary {
    pub id: Uuid,
    pub name: String,
    /// Names are unique per owner, so a summary is only unambiguous together
    /// with the account that owns it.
    pub owner_username: String,
    pub role: String,
    pub key_version: i32,
}

#[derive(Deserialize)]
pub struct JoinRequest {
    pub ott: String,
    pub refresher_token: String,
    #[serde(default)]
    pub mac_address: Option<String>,
}

/// Deliberately carries no account credentials. An invitation proves that the
/// holder was invited, not that they are the invitee, so it must never be
/// exchangeable for a session.
#[derive(Serialize)]
pub struct JoinResponse {
    pub project_id: Uuid,
    pub project_name: String,
    pub owner_username: String,
    pub key_version: i32,
    pub soft_token: String,
}

#[derive(Deserialize)]
pub struct RollbackRequest {
    pub commit_id: Uuid,
}
