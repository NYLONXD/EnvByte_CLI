use colored::Colorize;
use crate::utils::{
    config::{server_url, write_env_file, scan_env_files},
    local_store::{load_global_auth, LocalCommit, append_commit},
    crypto::{encrypt_env, decrypt_env, EncryptedPayload},
    mac::get_device_mac,
};

/// `greenbyte push --message "..."`
/// Scans for local .env* files, lets the user pick one, encrypts the content,
/// and sends only the encrypted string to the server.
pub async fn push(message: String) -> Result<(), String> {
    // ── Require login ──────────────────────────────────────────────────────
    let auth = load_global_auth()?;
    let auth_token = auth.auth_token
        .ok_or("Not logged in. Run `greenbyte login` first.")?;

    // ── Scan for .env* files ───────────────────────────────────────────────
    let env_files = scan_env_files();
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
        let idx: usize = choice.parse::<usize>()
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
    let env_content = std::fs::read_to_string(&selected_path)
        .map_err(|e| format!("Could not read {}: {}", filename, e))?;

    println!("  {} Read {} ({} bytes)", "●".green(), filename.cyan(), env_content.len());

    // ── Encrypt on the CLI side ────────────────────────────────────────────
    let mac = get_device_mac()?;
    let master_key = prompt_master_key()?;
    let encrypted = encrypt_env(&env_content, &master_key, &mac)?;

    println!("  {} Encrypted successfully", "●".green());

    // ── Send only the encrypted string to the server ───────────────────────
    let spinner = start_spinner("Pushing encrypted env to server...");

    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/users/me/env", server_url()))
        .bearer_auth(&auth_token)
        .json(&serde_json::json!({
            "filename": filename,
            "content": encrypted.data,
            "message": message,
        }))
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    spinner.finish_and_clear();

    if !res.status().is_success() {
        let msg: serde_json::Value = res.json().await.unwrap_or_default();
        return Err(format!("Push failed: {}",
            msg["message"].as_str()
                .or(msg["error"].as_str())
                .unwrap_or("unknown")));
    }

    // ── Also save as a local commit ────────────────────────────────────────
    let commit_id = uuid::Uuid::new_v4().to_string();
    let local_commit = LocalCommit {
        id: commit_id.clone(),
        message: message.clone(),
        timestamp: chrono::Utc::now(),
        env_snapshot: encrypted.data.clone(),
    };
    let _ = append_commit(local_commit);

    println!("{} Pushed successfully!", "✓".green().bold());
    println!("  File:    {}", filename.cyan());
    println!("  Commit:  {}", &commit_id[..8].cyan());
    println!("  Message: \"{}\"", message);
    Ok(())
}

/// `greenbyte pull`
/// Fetches the encrypted env string from the server and decrypts it locally.
pub async fn pull() -> Result<(), String> {
    // ── Require login ──────────────────────────────────────────────────────
    let auth = load_global_auth()?;
    let auth_token = auth.auth_token
        .ok_or("Not logged in. Run `greenbyte login` first.")?;

    // ── Fetch encrypted env from server ────────────────────────────────────
    let spinner = start_spinner("Pulling encrypted env from server...");

    let client = reqwest::Client::new();
    let res = client
        .get(format!("{}/users/me/env", server_url()))
        .bearer_auth(&auth_token)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    spinner.finish_and_clear();

    if !res.status().is_success() {
        return Err("Failed to fetch env. Check your connection and login.".to_string());
    }

    let data: serde_json::Value = res.json().await
        .map_err(|e| format!("Response parse error: {}", e))?;

    // The server returns an array of files — let the user pick if multiple
    let files = data.as_array()
        .ok_or("Server returned unexpected format.")?;

    if files.is_empty() {
        println!("{}", "No env files stored on the server yet.".yellow());
        println!("  Run `greenbyte push -m \"initial\"` to upload your .env.");
        return Ok(());
    }

    let selected = if files.len() == 1 {
        &files[0]
    } else {
        println!("{}", "Multiple env files found on server:".bold());
        for (i, f) in files.iter().enumerate() {
            let fname = f["filename"].as_str().unwrap_or("unknown");
            println!("  [{}] {}", (i + 1).to_string().cyan(), fname);
        }
        let choice = prompt(&format!("Select file (1-{}): ", files.len()))?;
        let idx: usize = choice.parse::<usize>()
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
    let filename = selected["filename"]
        .as_str()
        .unwrap_or(".env")
        .to_string();

    // ── Decrypt on the CLI side ────────────────────────────────────────────
    let mac = get_device_mac()?;
    let master_key = prompt_master_key()?;

    let payload = EncryptedPayload { data: encrypted_data };
    let decrypted = decrypt_env(&payload, &master_key, &mac)?;

    // ── Write to the local file ────────────────────────────────────────────
    if filename == ".env" {
        write_env_file(&decrypted)?;
    } else {
        std::fs::write(&filename, &decrypted)
            .map_err(|e| format!("Could not write {}: {}", filename, e))?;
    }

    println!("{} {} synced successfully!", "✓".green().bold(), filename.cyan());
    println!("  {} bytes decrypted and written.", decrypted.len());
    Ok(())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn prompt_master_key() -> Result<String, String> {
    rpassword::prompt_password("Master Key: ").map_err(|e| e.to_string())
}

fn prompt(label: &str) -> Result<String, String> {
    use std::io::{self, Write};
    print!("{}", label);
    io::stdout().flush().map_err(|e| e.to_string())?;
    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(|e| e.to_string())?;
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