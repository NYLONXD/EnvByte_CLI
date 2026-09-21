//! Project data key endpoints: reading your grants, and rotating.

use serde::{Deserialize, Serialize};

use crate::core::{
    api::client::{send_json, Session},
    crypto::keyring::Grant,
};

#[derive(Deserialize)]
struct GrantResponse {
    key_version: i32,
    wrapped_key: String,
}

#[derive(Serialize)]
pub struct GrantInput {
    pub user_id: String,
    pub wrapped_key: String,
}

#[derive(Serialize)]
pub struct RotatedFile {
    pub filename: String,
    pub content: String,
}

#[derive(Serialize)]
struct RotateRequest<'a> {
    grants: &'a [GrantInput],
    files: &'a [RotatedFile],
}

#[derive(Deserialize)]
pub struct RotationResult {
    pub key_version: i32,
    pub members_granted: usize,
    pub files_reencrypted: usize,
    pub grants_revoked: u64,
}

/// Every data key version the caller holds for this project.
pub async fn grants(session: &Session, project_id: &str) -> Result<Vec<Grant>, String> {
    let response: Vec<GrantResponse> = send_json(
        session.get(&format!("/projects/{project_id}/keys")),
        "Project key fetch",
    )
    .await?;
    Ok(response
        .into_iter()
        .map(|grant| Grant {
            key_version: grant.key_version,
            wrapped_key: grant.wrapped_key,
        })
        .collect())
}

/// Applies a rotation. The server accepts it only if it covers every current
/// member and every stored file.
pub async fn rotate(
    session: &Session,
    project_id: &str,
    grants: &[GrantInput],
    files: &[RotatedFile],
) -> Result<RotationResult, String> {
    send_json(
        session
            .post(&format!("/projects/{project_id}/key-rotations"))
            .json(&RotateRequest { grants, files }),
        "Key rotation",
    )
    .await
}
