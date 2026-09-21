//! Project endpoints.

use serde::{Deserialize, Serialize};

use crate::core::api::client::{send_json, Session};

#[derive(Serialize)]
struct CreateRequest<'a> {
    name: &'a str,
    wrapped_key: &'a str,
}

#[derive(Deserialize)]
pub struct CreatedProject {
    pub project_id: String,
    pub key_version: i32,
    pub soft_token: String,
}

#[derive(Deserialize)]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub owner_username: String,
    pub role: String,
    pub key_version: i32,
}

#[derive(Serialize)]
struct JoinRequest<'a> {
    ott: &'a str,
    refresher_token: &'a str,
    mac_address: Option<&'a str>,
}

#[derive(Deserialize)]
pub struct JoinedProject {
    pub project_id: String,
    pub project_name: String,
    pub owner_username: String,
    pub key_version: i32,
    pub soft_token: String,
}

pub async fn create(
    session: &Session,
    name: &str,
    wrapped_key: &str,
) -> Result<CreatedProject, String> {
    send_json(
        session
            .post("/projects")
            .json(&CreateRequest { name, wrapped_key }),
        "Project creation",
    )
    .await
}

pub async fn list(session: &Session) -> Result<Vec<ProjectSummary>, String> {
    send_json(session.get("/projects"), "Project list").await
}

/// The project's current key version, which a rotation increments.
pub async fn current_key_version(session: &Session, project_id: &str) -> Result<i32, String> {
    list(session)
        .await?
        .into_iter()
        .find(|project| project.id == project_id)
        .map(|project| project.key_version)
        .ok_or_else(|| {
            "This project is not in your project list. You may have been removed from it."
                .to_string()
        })
}

pub async fn join(
    session: &Session,
    ott: &str,
    refresher_token: &str,
    mac_address: Option<&str>,
) -> Result<JoinedProject, String> {
    send_json(
        session.post("/projects/join").json(&JoinRequest {
            ott,
            refresher_token,
            mac_address,
        }),
        "Project join",
    )
    .await
}

pub async fn rollback(session: &Session, project_id: &str, commit_id: &str) -> Result<(), String> {
    crate::core::api::client::send(
        session
            .post(&format!("/projects/{project_id}/rollback"))
            .json(&serde_json::json!({ "commit_id": commit_id })),
        "Rollback",
    )
    .await
}

pub async fn audit(session: &Session, project_id: &str) -> Result<Vec<serde_json::Value>, String> {
    send_json(
        session.get(&format!("/projects/{project_id}/audit")),
        "Audit log",
    )
    .await
}
