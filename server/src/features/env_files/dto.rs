//! Wire shapes for the environment-file endpoints.

use serde::{Deserialize, Deserializer, Serialize};
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
    /// The commit this content was edited from - the file's current commit
    /// as the client last saw it, or `null` for a file it believes is new.
    /// The push is refused if the server has moved on since, so one device
    /// cannot silently overwrite a change it never saw. Clients older than
    /// 0.5 omit it and keep last-write-wins.
    #[serde(default, deserialize_with = "present")]
    pub base_commit_id: Option<Option<Uuid>>,
}

/// Tells a field sent as `null` apart from one not sent at all: absent is
/// `None`, `null` is `Some(None)`.
fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn base_of(body: &str) -> Option<Option<Uuid>> {
        let request: PushRequest = serde_json::from_str(&format!(
            r#"{{"project_id":"00000000-0000-4000-8000-000000000000","filename":".env",
                "content":"x","message":"m","key_version":1{body}}}"#
        ))
        .unwrap();
        request.base_commit_id
    }

    #[test]
    fn an_absent_base_differs_from_a_null_one() {
        let id = "11111111-1111-4111-8111-111111111111";
        assert_eq!(base_of(""), None);
        assert_eq!(base_of(r#","base_commit_id":null"#), Some(None));
        assert_eq!(
            base_of(&format!(r#","base_commit_id":"{id}""#)),
            Some(Some(id.parse().unwrap()))
        );
    }
}
