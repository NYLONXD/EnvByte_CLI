//! Environment-file endpoints.

use serde::{Deserialize, Serialize};

use crate::core::api::client::{describe_error, send_json, Session};

#[derive(Deserialize)]
pub struct RemoteFile {
    pub filename: String,
    pub content: String,
    pub key_version: i32,
    /// The commit this content was stored under. Servers older than 0.5 do
    /// not send it.
    #[serde(default)]
    pub commit_id: Option<String>,
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
pub struct PushRequest<'a> {
    pub project_id: &'a str,
    pub commit_id: &'a str,
    pub filename: &'a str,
    pub content: &'a str,
    pub message: &'a str,
    pub key_version: i32,
    /// The commit the content was edited from, or `None` for a file this
    /// checkout believes is new. Always sent: `null` is a claim, not an
    /// omission.
    pub base_commit_id: Option<&'a str>,
}

#[derive(Deserialize)]
pub struct PushResult {
    pub commit_id: String,
    pub key_version: i32,
}

pub enum Pushed {
    Accepted(PushResult),
    /// The file moved on since `base_commit_id`; nothing was written.
    Stale,
}

/// `commit_id` is chosen by the client, so a retried push after a network
/// error is idempotent rather than a duplicate commit.
pub async fn push(session: &Session, request: &PushRequest<'_>) -> Result<Pushed, String> {
    let response = session
        .post("/users/me/env")
        .json(request)
        .send()
        .await
        .map_err(|e| format!("Push failed: {e}"))?;
    if response.status() == reqwest::StatusCode::CONFLICT {
        let body = response.text().await.unwrap_or_default();
        let value = serde_json::from_str::<serde_json::Value>(&body).unwrap_or_default();
        if value["code"] == "stale_base" {
            return Ok(Pushed::Stale);
        }
        let message = value["error"].as_str().unwrap_or("conflict");
        return Err(format!("Push failed (409 Conflict): {message}"));
    }
    if !response.status().is_success() {
        return Err(describe_error(response, "Push").await);
    }
    response
        .json()
        .await
        .map(Pushed::Accepted)
        .map_err(|e| format!("Push returned an unexpected response: {e}"))
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

/// One stored version of a file, current or not.
pub async fn commit(
    session: &Session,
    project_id: &str,
    commit_id: &str,
) -> Result<RemoteFile, String> {
    send_json(
        session.get(&format!("/projects/{project_id}/commits/{commit_id}")),
        "Version fetch",
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
