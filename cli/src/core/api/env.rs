//! Environment-file endpoints.

use serde::{Deserialize, Serialize};

use crate::core::api::client::{send_json, Session};

#[derive(Deserialize)]
pub struct RemoteFile {
    pub filename: String,
    pub content: String,
    pub key_version: i32,
}

#[derive(Deserialize)]
pub struct Commit {
    pub commit_id: String,
    pub filename: String,
    pub message: String,
    pub author: String,
    pub key_version: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize)]
struct PushRequest<'a> {
    project_id: &'a str,
    commit_id: &'a str,
    filename: &'a str,
    content: &'a str,
    message: &'a str,
    key_version: i32,
}

#[derive(Deserialize)]
pub struct PushResult {
    pub commit_id: String,
    pub key_version: i32,
}

/// `commit_id` is chosen by the client, so a retried push after a network
/// error is idempotent rather than a duplicate commit.
pub async fn push(
    session: &Session,
    project_id: &str,
    commit_id: &str,
    filename: &str,
    content: &str,
    message: &str,
    key_version: i32,
) -> Result<PushResult, String> {
    send_json(
        session.post("/users/me/env").json(&PushRequest {
            project_id,
            commit_id,
            filename,
            content,
            message,
            key_version,
        }),
        "Push",
    )
    .await
}

pub async fn pull(session: &Session, project_id: &str) -> Result<Vec<RemoteFile>, String> {
    send_json(
        session
            .get("/users/me/env")
            .query(&[("project_id", project_id)]),
        "Pull",
    )
    .await
}

pub async fn history(session: &Session, project_id: &str) -> Result<Vec<Commit>, String> {
    send_json(
        session.get(&format!("/projects/{project_id}/commits")),
        "History",
    )
    .await
}
