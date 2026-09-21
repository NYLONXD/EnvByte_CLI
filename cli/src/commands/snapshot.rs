//! `greenbyte commit` - an encrypted local snapshot, the offline half of
//! history.

use uuid::Uuid;

use crate::{
    commands::context::ProjectContext,
    core::{
        crypto::envelope,
        env_files::read_env_file,
        workspace::commit_log::{self, LocalCommit},
    },
    ui,
};

pub async fn commit(message: String) -> Result<(), String> {
    if message.trim().is_empty() {
        return Err("A commit message is required.".to_string());
    }
    let plaintext = read_env_file()?;
    let context = ProjectContext::load().await?;
    let keyring = context.keyring().await?;
    let (data_key, key_version) = keyring.current()?;

    let encrypted = envelope::encrypt(&plaintext, data_key, key_version)?;
    let id = Uuid::new_v4().to_string();

    commit_log::append(LocalCommit {
        id: id.clone(),
        message: message.trim().to_string(),
        timestamp: chrono::Utc::now(),
        env_snapshot: encrypted.data,
        filename: Some(".env".to_string()),
        project_id: Some(context.project_id),
        remote_commit_id: None,
        key_version: Some(key_version),
    })?;

    ui::success("Local snapshot saved.");
    ui::field("ID", &id);
    ui::field("Message", message.trim());
    ui::note(&format!(
        "Restore it with `greenbyte rollback --local {}`.",
        &id[..8]
    ));
    Ok(())
}
