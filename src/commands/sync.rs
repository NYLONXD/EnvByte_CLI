use crate::utils::{
    config::{
        ensure_file_size, http_client, response_error, scan_env_files, validate_env_filename,
        write_named_env_file,
    },
    crypto::{decrypt_env, encrypt_env, master_key_verifier, read_master_key, EncryptedPayload},
    local_store::{append_commit, load_config, LocalCommit},
    mac::get_device_mac,
};
use colored::Colorize;

/// `greenbyte push --message "..."`
/// Scans for local .env* files, lets the user pick one, encrypts the content,
/// and sends only the encrypted string to the server.
pub async fn push(message: String, requested_file: Option<String>) -> Result<(), String> {
    let config = load_config()?;
    let project_id = config
        .project_id
        .clone()
        .ok_or("The local project configuration has no project ID.")?;
    // ── Require login ──────────────────────────────────────────────────────
    let auth_token =
        crate::commands::auth::access_token(&config.server_url, config.auth_token.clone()).await?;

    // ── Scan for .env* files ───────────────────────────────────────────────
    let env_files = if let Some(filename) = requested_file {
        validate_env_filename(&filename)?;
        let path = std::path::PathBuf::from(filename);
        let kind = std::fs::symlink_metadata(&path)
            .map_err(|e| format!("Could not inspect {}: {e}", path.display()))?
            .file_type();
        if !kind.is_file() {
            return Err(format!("{} is not a regular file.", path.display()));
        }
        vec![path]
    } else {
        scan_env_files()
    };
    if env_files.is_empty() {
        return Err("No .env files found in the current directory.".to_string());
    }

    // ── Let user pick which file ───────────────────────────────────────────
    let selected_path = if env_files.len() == 1 {
        println!("  {} Found: {}", "●".green(), env_files[0].display());
        env_files[0].clone()
    } else {
        println!("{}", "Multiple .env files found:".bold());
        for (i, path) in env_files.iter().enumerate() {
            println!("  [{}] {}", (i + 1).to_string().cyan(), path.display());
        }
        let choice = prompt(&format!("Select file (1-{}): ", env_files.len()))?;
        let idx: usize = choice
            .parse::<usize>()
            .map_err(|_| "Invalid selection.")?
            .checked_sub(1)
            .ok_or("Invalid selection.")?;
        if idx >= env_files.len() {
            return Err("Selection out of range.".to_string());
        }
        env_files[idx].clone()
    };

    // ── Read the file as a string ──────────────────────────────────────────
    let filename = selected_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    ensure_file_size(&selected_path)?;
    let env_content = std::fs::read_to_string(&selected_path)
        .map_err(|e| format!("Could not read {}: {}", filename, e))?;

    println!(
        "  {} Read {} ({} bytes)",
        "●".green(),
        filename.cyan(),
        env_content.len()
    );

    // ── Encrypt on the CLI side ────────────────────────────────────────────
    let master_key = read_master_key()?;
    let encrypted = encrypt_env(&env_content, &master_key)?;

    println!("  {} Encrypted successfully", "●".green());

    // Generated before the request so a network retry remains idempotent and
    // local/remote history can share the same stable address.
    let commit_id = uuid::Uuid::new_v4().to_string();

    // ── Send only the encrypted string to the server ───────────────────────
    let spinner = start_spinner("Pushing encrypted env to server...");

    let client = http_client()?;
    let res = client
        .post(format!(
            "{}/users/me/env",
            config.server_url.trim_end_matches('/')
        ))
        .bearer_auth(&auth_token)
        .json(&serde_json::json!({
            "filename": filename,
            "content": encrypted.data,
            "message": message,
            "project_id": project_id,
            "commit_id": commit_id,
            "master_key_hash": master_key_verifier(&master_key),
        }))
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    spinner.finish_and_clear();

    if !res.status().is_success() {
        return Err(response_error(res, "Push").await);
    }

    let response: serde_json::Value = res.json().await.unwrap_or_default();
    let remote_commit_id = response["commit_id"]
        .as_str()
        .or_else(|| response["address"].as_str())
        .or_else(|| response["id"].as_str())
        .map(String::from);

    // ── Also save as a local commit ────────────────────────────────────────
    let local_commit = LocalCommit {
        id: commit_id.clone(),
        message: message.clone(),
        timestamp: chrono::Utc::now(),
        env_snapshot: encrypted.data.clone(),
        filename: Some(filename.clone()),
        project_id: Some(project_id),
        remote_commit_id: remote_commit_id.or_else(|| Some(commit_id.clone())),
    };
    append_commit(local_commit)?;

    println!("{} Pushed successfully!", "✓".green().bold());
    println!("  File:    {}", filename.cyan());
    println!("  Commit:  {}", &commit_id[..8].cyan());
    println!("  Message: \"{}\"", message);
    Ok(())
}

/// `greenbyte pull`
/// Fetches the encrypted env string from the server and decrypts it locally.
pub async fn pull(requested_file: Option<String>, force: bool) -> Result<(), String> {
    let config = load_config()?;
    let project_id = config
        .project_id
        .clone()
        .ok_or("The local project configuration has no project ID.")?;
    // ── Require login ──────────────────────────────────────────────────────
    let auth_token =
        crate::commands::auth::access_token(&config.server_url, config.auth_token.clone()).await?;

    // ── Fetch encrypted env from server ────────────────────────────────────
    let spinner = start_spinner("Pulling encrypted env from server...");

    let client = http_client()?;
    let res = client
        .get(format!(
            "{}/users/me/env",
            config.server_url.trim_end_matches('/')
        ))
        .query(&[("project_id", &project_id)])
        .bearer_auth(&auth_token)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    spinner.finish_and_clear();

    if !res.status().is_success() {
        return Err(response_error(res, "Pull").await);
    }

    let data: serde_json::Value = res
        .json()
        .await
        .map_err(|e| format!("Response parse error: {}", e))?;

    // The server returns an array of files — let the user pick if multiple
    let files = data
        .as_array()
        .ok_or("Server returned unexpected format.")?;

    if files.is_empty() {
        println!("{}", "No env files stored on the server yet.".yellow());
        println!("  Run `greenbyte push -m \"initial\"` to upload your .env.");
        return Ok(());
    }

    let selected = if let Some(filename) = requested_file {
        validate_env_filename(&filename)?;
        files
            .iter()
            .find(|file| file["filename"].as_str() == Some(filename.as_str()))
            .ok_or_else(|| format!("No remote environment file named {filename}."))?
    } else if files.len() == 1 {
        &files[0]
    } else {
        println!("{}", "Multiple env files found on server:".bold());
        for (i, f) in files.iter().enumerate() {
            let fname = f["filename"].as_str().unwrap_or("unknown");
            println!("  [{}] {}", (i + 1).to_string().cyan(), fname);
        }
        let choice = prompt(&format!("Select file (1-{}): ", files.len()))?;
        let idx: usize = choice
            .parse::<usize>()
            .map_err(|_| "Invalid selection.")?
            .checked_sub(1)
            .ok_or("Invalid selection.")?;
        if idx >= files.len() {
            return Err("Selection out of range.".to_string());
        }
        &files[idx]
    };

    let encrypted_data = selected["content"]
        .as_str()
        .ok_or("Server returned no encrypted content.")?
        .to_string();
    let filename = selected["filename"].as_str().unwrap_or(".env").to_string();

    // ── Decrypt on the CLI side ────────────────────────────────────────────
    let mac = get_device_mac();
    let master_key = read_master_key()?;

    let payload = EncryptedPayload {
        data: encrypted_data,
    };
    let decrypted = decrypt_env(&payload, &master_key, mac.as_deref())?;

    // ── Write to the local file ────────────────────────────────────────────
    if std::path::Path::new(&filename).exists() && !force {
        let confirmation = prompt(&format!("{filename} already exists. Overwrite it? [y/N]: "))?;
        if confirmation.to_lowercase() != "y" {
            println!("Aborted.");
            return Ok(());
        }
    }
    write_named_env_file(&filename, &decrypted)?;

    println!(
        "{} {} synced successfully!",
        "✓".green().bold(),
        filename.cyan()
    );
    println!("  {} bytes decrypted and written.", decrypted.len());
    Ok(())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn prompt(label: &str) -> Result<String, String> {
    use std::io::{self, Write};
    print!("{}", label);
    io::stdout().flush().map_err(|e| e.to_string())?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| e.to_string())?;
    Ok(input.trim().to_string())
}

fn start_spinner(msg: &str) -> indicatif::ProgressBar {
    use indicatif::{ProgressBar, ProgressStyle};
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    pb
}
