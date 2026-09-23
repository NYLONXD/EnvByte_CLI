//! `envbyte push` and `envbyte pull`.

use std::path::PathBuf;

use colored::Colorize;

use crate::{
    commands::context::ProjectContext,
    core::{
        api::env,
        crypto::{envelope, open_payload, EncryptedPayload},
        device,
        env_files::{
            ensure_file_size, scan_env_files, validate_env_filename, write_named_env_file,
        },
        workspace::commit_log::{self, LocalCommit},
    },
    ui,
};

/// Encrypts a local `.env*` under the project key and sends only ciphertext.
pub async fn push(message: String, requested_file: Option<String>) -> Result<(), String> {
    if message.trim().is_empty() {
        return Err("A commit message is required.".to_string());
    }
    let context = ProjectContext::load().await?;
    let keyring = context.keyring().await?;
    let (data_key, key_version) = keyring.current()?;

    let candidates = local_candidates(requested_file)?;
    let index = ui::choose(".env files found:", &candidates, |path| {
        path.display().to_string()
    })?;
    let selected = &candidates[index];
    let filename = selected
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    ensure_file_size(selected)?;
    let plaintext =
        std::fs::read_to_string(selected).map_err(|e| format!("Could not read {filename}: {e}"))?;
    ui::step(&format!(
        "Read {} ({} bytes)",
        filename.cyan(),
        plaintext.len()
    ));

    let encrypted = envelope::encrypt(&plaintext, data_key, key_version)?;
    ui::step(&format!("Encrypted under project key v{key_version}"));

    // Chosen before the request, so a retry after a network error is
    // idempotent and local history shares the remote address.
    let commit_id = uuid::Uuid::new_v4().to_string();

    let bar = ui::spinner("Pushing...");
    let result = env::push(
        &context.session,
        &context.project_id,
        &commit_id,
        &filename,
        &encrypted.data,
        message.trim(),
        key_version,
    )
    .await;
    bar.finish_and_clear();
    let result = result?;

    commit_log::append(LocalCommit {
        id: commit_id.clone(),
        message: message.trim().to_string(),
        timestamp: chrono::Utc::now(),
        env_snapshot: encrypted.data,
        filename: Some(filename.clone()),
        project_id: Some(context.project_id.clone()),
        remote_commit_id: Some(result.commit_id),
        key_version: Some(key_version),
    })?;

    ui::success("Pushed.");
    ui::field("File", &filename);
    ui::field("Commit", &commit_id[..8]);
    ui::field("Key version", &result.key_version.to_string());
    ui::field("Message", message.trim());
    Ok(())
}

/// Fetches ciphertext and decrypts it with whichever held key each file names.
pub async fn pull(requested_file: Option<String>, force: bool) -> Result<(), String> {
    let context = ProjectContext::load().await?;
    let keyring = context.keyring().await?;

    let bar = ui::spinner("Pulling...");
    let files = env::pull(&context.session, &context.project_id).await;
    bar.finish_and_clear();
    let files = files?;

    if files.is_empty() {
        ui::warn("No environment files stored for this project yet.");
        ui::note("Run `envbyte push -m \"initial\"` to upload one.");
        return Ok(());
    }

    let selected = match &requested_file {
        Some(name) => {
            validate_env_filename(name)?;
            files
                .iter()
                .find(|file| &file.filename == name)
                .ok_or_else(|| format!("No remote environment file named {name}."))?
        }
        None => {
            let index = ui::choose("Environment files on the server:", &files, |file| {
                format!("{} (key v{})", file.filename, file.key_version)
            })?;
            &files[index]
        }
    };

    let payload = EncryptedPayload {
        data: selected.content.clone(),
    };
    let plaintext = open_payload(&keyring, &payload, device::get_device_mac().as_deref())?;

    if std::path::Path::new(&selected.filename).exists()
        && !force
        && !ui::confirm(&format!(
            "{} already exists. Overwrite it?",
            selected.filename
        ))?
    {
        println!("Aborted.");
        return Ok(());
    }
    write_named_env_file(&selected.filename, &plaintext)?;

    ui::success(&format!("{} synced.", selected.filename.cyan()));
    ui::field("Bytes", &plaintext.len().to_string());
    ui::field("Key version", &selected.key_version.to_string());

    // Record what this checkout last saw, so `status` can say something useful.
    let mut config = context.config;
    if config.key_version != Some(keyring.current_version()) {
        config.key_version = Some(keyring.current_version());
        crate::core::workspace::project_config::save(&config)?;
    }
    Ok(())
}

/// The `.env*` files a push may choose from.
fn local_candidates(requested: Option<String>) -> Result<Vec<PathBuf>, String> {
    let candidates = match requested {
        Some(name) => {
            validate_env_filename(&name)?;
            let path = PathBuf::from(name);
            let kind = std::fs::symlink_metadata(&path)
                .map_err(|e| format!("Could not inspect {}: {e}", path.display()))?
                .file_type();
            // Refuse symlinks: a link could point outside the project at
            // something the user did not mean to publish.
            if !kind.is_file() {
                return Err(format!("{} is not a regular file.", path.display()));
            }
            vec![path]
        }
        None => scan_env_files(),
    };
    if candidates.is_empty() {
        return Err("No .env files found in the current directory.".to_string());
    }
    Ok(candidates)
}
