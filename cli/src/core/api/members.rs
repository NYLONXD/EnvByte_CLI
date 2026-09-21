//! Collaborator endpoints.

use serde::{Deserialize, Serialize};

use crate::core::api::client::{send, send_json, Session};

#[derive(Deserialize)]
pub struct Member {
    pub user_id: String,
    pub username: String,
    pub email: String,
    pub role: String,
    pub public_key: Option<String>,
    pub key_version: Option<i32>,
}

#[derive(Deserialize)]
pub struct Invitee {
    pub username: String,
    pub public_key: String,
}

#[derive(Serialize)]
struct LookupRequest<'a> {
    email: &'a str,
}

#[derive(Serialize)]
struct InviteRequest<'a> {
    email: &'a str,
    wrapped_key: &'a str,
}

#[derive(Deserialize)]
pub struct InviteResult {
    #[serde(default = "default_ttl")]
    pub expires_in_hours: i64,
}

fn default_ttl() -> i64 {
    24
}

pub async fn list(session: &Session, project_id: &str) -> Result<Vec<Member>, String> {
    send_json(
        session.get(&format!("/projects/{project_id}/collaborators")),
        "Collaborator list",
    )
    .await
}

/// Resolves the identity key to seal the project key to, before inviting.
pub async fn lookup(session: &Session, project_id: &str, email: &str) -> Result<Invitee, String> {
    send_json(
        session
            .post(&format!("/projects/{project_id}/collaborators/lookup"))
            .json(&LookupRequest { email }),
        "Collaborator lookup",
    )
    .await
}

pub async fn invite(
    session: &Session,
    project_id: &str,
    email: &str,
    wrapped_key: &str,
) -> Result<InviteResult, String> {
    send_json(
        session
            .post(&format!("/projects/{project_id}/collaborators"))
            .json(&InviteRequest { email, wrapped_key }),
        "Collaborator invite",
    )
    .await
}

pub async fn remove(session: &Session, project_id: &str, user_id: &str) -> Result<(), String> {
    send(
        session.delete(&format!("/projects/{project_id}/collaborators/{user_id}")),
        "Collaborator removal",
    )
    .await
}

pub async fn change_role(
    session: &Session,
    project_id: &str,
    user_id: &str,
    role: &str,
) -> Result<(), String> {
    send(
        session
            .patch(&format!("/projects/{project_id}/collaborators/{user_id}"))
            .json(&serde_json::json!({ "role": role })),
        "Role change",
    )
    .await
}
