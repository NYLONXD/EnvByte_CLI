use colored::Colorize;
use crate::utils::{
    config::{server_url, read_env_file, write_env_file},
    local_store::{load_config, load_global_auth, LocalCommit, append_commit},
    crypto::{encrypt_env, decrypt_env, EncryptedPayload},
    mac::get_device_mac,
};

/// `greenbyte push --message "..."`
/// Encrypts local .env and sends it to the server.
pub async fn push(message: String) -> Result<(), String> {
    let config = load_config()?;

    // Use .clone() so config is not partially moved before get_auth_token borrows it
    let project_id = config.project_id.clone()
        .ok_or("No project linked. Run `greenbyte create` or `greenbyte init`.")?;
    let auth_token = get_auth_token(&config)?;

    let env_content = read_env_file()?;
    let mac = get_device_mac()?;
    let master_key = prompt_master_key()?;

    let encrypted = encrypt_env(&env_content, &master_key, &mac)?;

    let spinner = start_spinner("Pushing .env to server...");

    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/projects/{}/env", server_url(), project_id))
        .bearer_auth(&auth_token)
        .json(&serde_json::json!({
            "encrypted_env": encrypted.data,
            "message": message,
        }))
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    spinner.finish_and_clear();

    if !res.status().is_success() {
        let msg: serde_json::Value = res.json().await.unwrap_or_default();
        return Err(format!("Push failed: {}", msg["error"].as_str().unwrap_or("unknown")));
    }

    let data: serde_json::Value = res.json().await.unwrap_or_default();
    let commit_id = data["commit_id"].as_str().unwrap_or("unknown");

    // Also save as a local commit
    let local_commit = LocalCommit {
        id: commit_id.to_string(),
        message: message.clone(),
        timestamp: chrono::Utc::now(),
        env_snapshot: encrypted.data.clone(),
    };
    let _ = append_commit(local_commit);

    println!("{} Pushed successfully!", "✓".green().bold());
    println!("  Commit: {}", commit_id.cyan());
    println!("  Message: \"{}\"", message);
    Ok(())
}

/// `greenbyte pull`
/// Fetches encrypted .env from server and decrypts it locally.
pub async fn pull() -> Result<(), String> {
    let config = load_config()?;

    // Use .clone() so config is not partially moved before get_auth_token borrows it
    let project_id = config.project_id.clone()
        .ok_or("No project linked. Run `greenbyte create` or `greenbyte init`.")?;
    let auth_token = get_auth_token(&config)?;

    let spinner = start_spinner("Pulling .env from server...");

    let client = reqwest::Client::new();
    let res = client
        .get(format!("{}/projects/{}/env", server_url(), project_id))
        .bearer_auth(&auth_token)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    spinner.finish_and_clear();

    if !res.status().is_success() {
        return Err("Failed to fetch .env. Check your connection and token.".to_string());
    }

    let data: serde_json::Value = res.json().await
        .map_err(|e| format!("Response parse error: {}", e))?;

    let encrypted_data = data["encrypted_env"]
        .as_str()
        .ok_or("Server returned no encrypted data.")?
        .to_string();

    let mac = get_device_mac()?;
    let master_key = prompt_master_key()?;

    let payload = EncryptedPayload { data: encrypted_data };
    let decrypted = decrypt_env(&payload, &master_key, &mac)?;

    write_env_file(&decrypted)?;

    println!("{} .env synced successfully!", "✓".green().bold());
    if let Some(commit_id) = data["commit_id"].as_str() {
        println!("  At commit: {}", commit_id.cyan());
    }
    Ok(())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn get_auth_token(config: &crate::utils::local_store::LocalConfig) -> Result<String, String> {
    if let Some(token) = &config.auth_token {
        return Ok(token.clone());
    }
    // Fall back to global auth file
    let global = load_global_auth()?;
    global.auth_token.ok_or("Not logged in. Run `greenbyte login`.".to_string())
}

fn prompt_master_key() -> Result<String, String> {
    rpassword::prompt_password("Master Key: ").map_err(|e| e.to_string())
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