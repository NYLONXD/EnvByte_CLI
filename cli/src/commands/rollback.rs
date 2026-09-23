//! `envbyte rollback` - restore a previous state, on the server or locally.

use colored::Colorize;

use crate::{
    commands::context::ProjectContext,
    core::{
        api::projects,
        crypto::{open_payload, EncryptedPayload},
        device,
        env_files::write_named_env_file,
        workspace::commit_log,
    },
    ui,
};

pub async fn rollback(address: Option<String>, local: Option<String>) -> Result<(), String> {
    match (address, local) {
        (Some(commit), None) => remote(commit).await,
        (None, Some(commit)) => offline(commit).await,
        (Some(_), Some(_)) => Err("Use either --address or --local, not both.".to_string()),
        (None, None) => Err(
            "Provide --address <id> to roll the server back, or --local <id> to restore from a \
             local snapshot."
                .to_string(),
        ),
    }
}

async fn remote(commit_id: String) -> Result<(), String> {
    let context = ProjectContext::load().await?;
    let short = &commit_id[..8.min(commit_id.len())];

    if !ui::confirm(&format!(
        "Roll {} back to commit {short}? This replaces the current server state",
        context.config.qualified_name()
    ))? {
        println!("Aborted.");
        return Ok(());
    }

    let bar = ui::spinner("Rolling back...");
    let result = projects::rollback(&context.session, &context.project_id, &commit_id).await;
    bar.finish_and_clear();
    result?;

    ui::success(&format!("Rolled back to commit {}.", short.cyan()));
    ui::note("Run `envbyte pull` to sync your local .env.");
    Ok(())
}

async fn offline(prefix: String) -> Result<(), String> {
    let store = commit_log::load()?;
    let commit = store
        .commits
        .iter()
        .find(|commit| commit.id == prefix || commit.id.starts_with(&prefix))
        .ok_or_else(|| format!("No local snapshot with an ID starting '{prefix}'."))?;

    ui::heading("Restore local snapshot");
    ui::field("ID", &commit.id[..8]);
    ui::field("Message", &commit.message);
    ui::field(
        "Taken",
        &commit.timestamp.format("%Y-%m-%d %H:%M UTC").to_string(),
    );

    let filename = commit
        .filename
        .clone()
        .unwrap_or_else(|| ".env".to_string());
    if !ui::confirm(&format!("This overwrites your local {filename}. Continue?"))? {
        println!("Aborted.");
        return Ok(());
    }

    // Decrypting needs the project key, which means a live session - a local
    // snapshot is ciphertext, not a plaintext backup.
    let context = ProjectContext::load().await?;
    let keyring = context.keyring().await?;
    let plaintext = open_payload(
        &keyring,
        &EncryptedPayload {
            data: commit.env_snapshot.clone(),
        },
        device::get_device_mac().as_deref(),
    )?;
    write_named_env_file(&filename, &plaintext)?;

    ui::success(&format!(
        "{} restored from snapshot {}.",
        filename.cyan(),
        commit.id[..8].cyan()
    ));
    Ok(())
}
