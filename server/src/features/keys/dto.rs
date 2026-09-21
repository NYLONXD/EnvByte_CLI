//! Wire shapes for the key endpoints.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct RotateRequest {
    /// The new data key sealed once per remaining member. Must cover every
    /// current member exactly - a missing entry would lock a colleague out.
    pub grants: Vec<GrantInput>,
    /// Every environment file in the project, re-encrypted under the new key.
    /// Anything left behind would stay readable with the retired key.
    pub files: Vec<RotatedFile>,
}

#[derive(Deserialize)]
pub struct GrantInput {
    pub user_id: Uuid,
    pub wrapped_key: String,
}

#[derive(Deserialize)]
pub struct RotatedFile {
    pub filename: String,
    pub content: String,
}

#[derive(Serialize)]
pub struct RotateResponse {
    pub key_version: i32,
    pub members_granted: usize,
    pub files_reencrypted: usize,
    pub grants_revoked: u64,
}
