//! Wire shapes for the membership endpoints.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct LookupRequest {
    pub email: String,
}

#[derive(Serialize)]
pub struct LookupResponse {
    pub user_id: Uuid,
    pub username: String,
    pub public_key: String,
}

#[derive(Deserialize)]
pub struct InviteRequest {
    pub email: String,
    /// The project data key sealed to the invitee's published identity key.
    /// Without it the invitee joins but cannot read anything, so the invite
    /// and the grant are written in one transaction.
    pub wrapped_key: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct MemberResponse {
    pub user_id: Uuid,
    pub username: String,
    pub email: String,
    pub role: String,
    /// Needed by an admin to re-seal the data key during a rotation.
    pub public_key: Option<String>,
    /// Highest key version this member currently holds, so a rotation can
    /// report who would be left behind before it runs.
    pub key_version: Option<i32>,
    pub joined_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct ChangeRoleRequest {
    pub role: String,
}
