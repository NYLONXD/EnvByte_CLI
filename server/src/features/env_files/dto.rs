//! Wire shapes for the environment-file endpoints.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct PushRequest {
    pub project_id: Uuid,
    #[serde(default)]
    pub commit_id: Option<Uuid>,
    pub filename: String,
    pub content: String,
    pub message: String,
    /// Which project data key this content was encrypted under. The server
    /// cannot decrypt, so this is the only way to stop a client holding a
    /// retired key from overwriting the file with a payload the team can no
    /// longer read.
    pub key_version: i32,
}

#[derive(Serialize)]
pub struct PushResponse {
    pub commit_id: Uuid,
    pub key_version: i32,
}

#[derive(Deserialize)]
pub struct PullQuery {
    pub project_id: Uuid,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct CommitResponse {
    pub commit_id: Uuid,
    pub filename: String,
    pub message: String,
    pub author: String,
    pub key_version: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
