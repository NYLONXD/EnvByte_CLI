//! `envbyte push` and `envbyte pull`.
//!
//! Each checkout remembers the server commit its files were last synced at.
//! A push names that commit, and the server refuses it if anyone has pushed
//! since; the two edits are then merged here, key by key, and pushed again.
//! The server cannot merge for us - it only ever sees ciphertext.

use std::{
    collections::HashMap,
    io::IsTerminal,
    path::{Path, PathBuf},
};

use colored::Colorize;

use crate::{
    commands::context::ProjectContext,
    core::{
        api::env::{self, PushRequest, Pushed, RemoteFile},
        crypto::{envelope, keyring::Keyring, open_payload, EncryptedPayload},
        device,
        env_files::{
            ensure_file_size, scan_env_files, validate_env_filename, write_named_env_file,
        },
        env_merge::{self, Side},
        workspace::{
            commit_log::{self, LocalCommit},
            project_config::{self, ProjectConfig},
        },
    },
    ui,
};

/// How many times a push merges and retries before giving up on a file that
/// keeps changing underneath it.
const PUSH_ATTEMPTS: usize = 3;

/// Encrypts a local `.env*` under the project key and sends only ciphertext.
pub async fn push(
    message: String,
    requested_file: Option<String>,
    force: bool,
) -> Result<(), String> {
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
    let mut plaintext =
        std::fs::read_to_string(selected).map_err(|e| format!("Could not read {filename}: {e}"))?;
    ui::step(&format!(
        "Read {} ({} bytes)",
        filename.cyan(),
        plaintext.len()
    ));

    let mut config = context.config.clone();
    let mut base = last_synced(&config, &context.project_id, &filename);

    for attempt in 0..PUSH_ATTEMPTS {
        let encrypted = envelope::encrypt(&plaintext, data_key, key_version)?;
        if attempt == 0 {
            ui::step(&format!("Encrypted under project key v{key_version}"));
        }

        // Chosen before the request, so a retry after a network error is
        // idempotent and local history shares the remote address.
        let commit_id = uuid::Uuid::new_v4().to_string();

        let bar = ui::spinner("Pushing...");
        let outcome = env::push(
            &context.session,
            &PushRequest {
                project_id: &context.project_id,
                commit_id: &commit_id,
                filename: &filename,
                content: &encrypted.data,
                message: message.trim(),
                key_version,
                base_commit_id: base.as_deref(),
            },
        )
        .await;
        bar.finish_and_clear();

        let result = match outcome? {
            Pushed::Accepted(result) => result,
            Pushed::Stale => {
                let server = remote_file(&context, &filename).await?;
                let server_commit = server.as_ref().and_then(|file| file.commit_id.clone());
                if force {
                    ui::warn(&format!(
                        "{filename} changed on the server; replacing it with yours (--force)."
                    ));
                } else {
                    let reconciled = reconcile(
                        &context,
                        &keyring,
                        &filename,
                        base.as_deref(),
                        &plaintext,
                        server.as_ref(),
                    )
                    .await?;
                    match reconciled {
                        Reconciled::Push(merged) => {
                            if merged != plaintext {
                                write_named_env_file(&filename, &merged)?;
                                plaintext = merged;
                            }
                        }
                        Reconciled::UpToDate => {
                            record_synced(&mut config, &filename, server_commit)?;
                            ui::success(&format!(
                                "{} is already up to date on the server.",
                                filename.cyan()
                            ));
                            return Ok(());
                        }
                        Reconciled::Nothing => return Ok(()),
                    }
                    // The local file now holds everything the server has, so
                    // it counts as synced at the server's commit even if the
                    // push below fails.
                    record_synced(&mut config, &filename, server_commit.clone())?;
                }
                base = server_commit;
                continue;
            }
        };

        commit_log::append(LocalCommit {
            id: commit_id.clone(),
            message: message.trim().to_string(),
            timestamp: chrono::Utc::now(),
            env_snapshot: encrypted.data,
            filename: Some(filename.clone()),
            project_id: Some(context.project_id.clone()),
            remote_commit_id: Some(result.commit_id.clone()),
            key_version: Some(key_version),
        })?;
        record_synced(&mut config, &filename, Some(result.commit_id))?;

        ui::success("Pushed.");
        ui::field("File", &filename);
        ui::field("Commit", &commit_id[..8]);
        ui::field("Key version", &result.key_version.to_string());
        ui::field("Message", message.trim());
        return Ok(());
    }
    Err(format!(
        "{filename} kept changing on the server while merging. Run `envbyte push` again."
    ))
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
    let filename = selected.filename.as_str();
    let server_text = decrypt(&keyring, &selected.content)?;
    let mut config = context.config.clone();

    let path = Path::new(filename);
    let replacement = if !path.exists() || force {
        Some(server_text)
    } else {
        ensure_file_size(path)?;
        let local =
            std::fs::read_to_string(path).map_err(|e| format!("Could not read {filename}: {e}"))?;
        let base = last_synced(&config, &context.project_id, filename);
        if env_merge::equivalent(&local, &server_text) {
            None
        } else {
            match base_text(&context, &keyring, base.as_deref()).await {
                // Nothing edited locally: the server copy simply replaces it.
                Some(original) if env_merge::equivalent(&local, &original) => Some(server_text),
                // Only the local copy changed, so it already holds everything
                // the server has.
                Some(original) if env_merge::equivalent(&server_text, &original) => {
                    ui::note(&format!(
                        "{filename} has local edits the server does not have yet; nothing to pull."
                    ));
                    ui::note("Run `envbyte push` to share them.");
                    None
                }
                original => {
                    match settle_local_edits(filename, original.as_deref(), &local, &server_text)? {
                        Some(text) => Some(text),
                        None => {
                            println!("Aborted. {filename} was left as it was.");
                            return Ok(());
                        }
                    }
                }
            }
        }
    };

    match &replacement {
        Some(text) => {
            write_named_env_file(filename, text)?;
            ui::success(&format!("{} synced.", filename.cyan()));
            ui::field("Bytes", &text.len().to_string());
        }
        None => ui::success(&format!(
            "{} already has everything on the server.",
            filename.cyan()
        )),
    }
    ui::field("Key version", &selected.key_version.to_string());

    // Record what this checkout last saw: `status` shows the key version, and
    // the next push names the commit as its base.
    config.key_version = Some(keyring.current_version());
    record_synced(&mut config, filename, selected.commit_id.clone())
}

/// What a push refused as stale should do next.
enum Reconciled {
    /// Push this text on top of the server's current commit.
    Push(String),
    /// The server already holds exactly the local content.
    UpToDate,
    /// Nothing to push; the reason has been shown.
    Nothing,
}

/// Brings a stale push up to date with the server copy: merges where both
/// changed, and stops where there is nothing of ours left to push.
async fn reconcile(
    context: &ProjectContext,
    keyring: &Keyring,
    filename: &str,
    base: Option<&str>,
    local: &str,
    server: Option<&RemoteFile>,
) -> Result<Reconciled, String> {
    let Some(server) = server else {
        // Gone from the server since; push it as a new file.
        return Ok(Reconciled::Push(local.to_string()));
    };
    let server_text = decrypt(keyring, &server.content)?;
    if env_merge::equivalent(local, &server_text) {
        return Ok(Reconciled::UpToDate);
    }
    let original = base_text(context, keyring, base).await;
    if let Some(original) = &original {
        if env_merge::equivalent(local, original) {
            ui::warn(&format!(
                "{filename} has no edits since you last synced it, but the server has newer ones."
            ));
            ui::note("Run `envbyte pull` to get them.");
            return Ok(Reconciled::Nothing);
        }
        // Re-encrypted by a key rotation, or rolled back to the same lines:
        // nothing to merge in.
        if env_merge::equivalent(&server_text, original) {
            return Ok(Reconciled::Push(local.to_string()));
        }
    }

    ui::warn(&format!(
        "{filename} changed on the server since this checkout last synced it."
    ));
    let merged = merge(
        filename,
        original.as_deref(),
        local,
        &server_text,
        "pass --force to replace the server copy with yours",
        &format!("Save the merged {filename} and push it?"),
    )?;
    Ok(match merged {
        Some(text) => Reconciled::Push(text),
        None => {
            println!("Aborted. Nothing was pushed.");
            Reconciled::Nothing
        }
    })
}

/// Asks what to do with a local file that differs from the server copy a
/// pull is about to write. `None` if the user backs out.
fn settle_local_edits(
    filename: &str,
    original: Option<&str>,
    local: &str,
    server: &str,
) -> Result<Option<String>, String> {
    if !std::io::stdin().is_terminal() {
        return Err(format!(
            "{filename} differs from the server copy. Run `envbyte pull` in a terminal to merge, \
             or pass --force to replace it with the server copy."
        ));
    }
    if original.is_some() {
        ui::warn(&format!(
            "{filename} has local edits, and the server has newer changes."
        ));
    } else {
        ui::warn(&format!("{filename} differs from the server copy."));
    }
    let options = [
        "Merge the two, key by key",
        "Discard local edits and take the server copy",
        "Cancel",
    ];
    match ui::choose("What should happen?", &options, |option| option.to_string())? {
        0 => merge(
            filename,
            original,
            local,
            server,
            "pass --force to replace it with the server copy",
            &format!("Save the merged {filename}?"),
        ),
        1 => Ok(Some(server.to_string())),
        _ => Ok(None),
    }
}

/// Merges `local` with `server`, both edited from `original`, asking about
/// every key both changed. `None` if the user backs out.
fn merge(
    filename: &str,
    original: Option<&str>,
    local: &str,
    server: &str,
    force_hint: &str,
    question: &str,
) -> Result<Option<String>, String> {
    let plan = env_merge::plan(original, local, server);
    ui::heading(&format!("Merging {filename}"));
    for change in &plan.from_server {
        ui::step(&format!(
            "{} {} on the server",
            change.key.cyan(),
            change.kind.describe()
        ));
    }
    for change in &plan.from_local {
        ui::step(&format!(
            "{} {} by you",
            change.key.cyan(),
            change.kind.describe()
        ));
    }

    let interactive = std::io::stdin().is_terminal();
    if !plan.conflicts.is_empty() && !interactive {
        let keys: Vec<&str> = plan
            .conflicts
            .iter()
            .map(|conflict| conflict.key.as_str())
            .collect();
        return Err(format!(
            "Both you and the server changed {}. Run this in a terminal to choose, or {force_hint}.",
            keys.join(", ")
        ));
    }

    let mut choices = HashMap::new();
    for conflict in &plan.conflicts {
        println!();
        ui::warn(&format!(
            "{} was changed both by you and on the server.",
            conflict.key.cyan()
        ));
        let options = [
            (Side::Local, "keep yours", conflict.local.as_deref()),
            (
                Side::Server,
                "take the server's",
                conflict.server.as_deref(),
            ),
        ];
        let index = ui::choose("Which value should win?", &options, |(_, label, value)| {
            format!("{label:<18} {}", preview(*value))
        })?;
        choices.insert(conflict.key.clone(), options[index].0);
    }

    if interactive && !ui::confirm(question)? {
        return Ok(None);
    }
    Ok(Some(plan.render(|conflict| choices[&conflict.key])))
}

/// Enough of a value to tell two apart, without printing a secret in full.
fn preview(value: Option<&str>) -> String {
    let Some(value) = value else {
        return "(removed)".to_string();
    };
    let chars: Vec<char> = value.chars().collect();
    let length = chars.len();
    if length <= 8 {
        return format!("{} ({length} chars)", "*".repeat(length));
    }
    let head: String = chars[..3].iter().collect();
    let tail: String = chars[length - 2..].iter().collect();
    format!("{head}...{tail} ({length} chars)")
}

/// The version both sides started from, if it can still be read. Without it
/// every key the two copies disagree on has to be settled by hand.
async fn base_text(
    context: &ProjectContext,
    keyring: &Keyring,
    base: Option<&str>,
) -> Option<String> {
    let base = base?;
    let fetched = env::commit(&context.session, &context.project_id, base).await;
    match fetched.and_then(|file| decrypt(keyring, &file.content)) {
        Ok(text) => Some(text),
        Err(error) => {
            ui::note(&format!(
                "Could not read the version you last synced ({error}), so every difference \
                 needs a decision."
            ));
            None
        }
    }
}

async fn remote_file(
    context: &ProjectContext,
    filename: &str,
) -> Result<Option<RemoteFile>, String> {
    let files = env::pull(&context.session, &context.project_id).await?;
    Ok(files.into_iter().find(|file| file.filename == filename))
}

fn decrypt(keyring: &Keyring, content: &str) -> Result<String, String> {
    let payload = EncryptedPayload {
        data: content.to_string(),
    };
    open_payload(keyring, &payload, device::get_device_mac().as_deref())
}

/// The commit this checkout last synced `filename` at. Checkouts made before
/// 0.5 never recorded one; their last push, in the local log, is the next
/// best guess.
fn last_synced(config: &ProjectConfig, project_id: &str, filename: &str) -> Option<String> {
    if let Some(commit) = config.synced_commits.get(filename) {
        return Some(commit.clone());
    }
    commit_log::load()
        .ok()?
        .commits
        .into_iter()
        .rev()
        .find(|commit| {
            commit.project_id.as_deref() == Some(project_id)
                && commit.filename.as_deref() == Some(filename)
                && commit.remote_commit_id.is_some()
        })
        .and_then(|commit| commit.remote_commit_id)
}

fn record_synced(
    config: &mut ProjectConfig,
    filename: &str,
    commit: Option<String>,
) -> Result<(), String> {
    match commit {
        Some(commit) => config.synced_commits.insert(filename.to_string(), commit),
        None => config.synced_commits.remove(filename),
    };
    project_config::save(config)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_never_show_a_whole_secret() {
        assert_eq!(preview(None), "(removed)");
        assert_eq!(preview(Some("short")), "***** (5 chars)");
        assert_eq!(preview(Some("sk_live_0123456789")), "sk_...89 (18 chars)");
    }
}
