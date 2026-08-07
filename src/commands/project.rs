use crate::utils::{
    config::{http_client, response_error, server_url, validate_server_url},
    crypto::generate_token,
    local_store::{config_exists, save_config, save_global_auth, GlobalAuth, LocalConfig},
    mac::get_device_mac,
};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

#[derive(Serialize)]
struct CreateProjectRequest {
    name: String,
    master_key_hash: String, // we send a hash of master key, never the key itself
}

#[derive(Deserialize)]
struct CreateProjectResponse {
    project_id: String,
    soft_token: String,
}

/// `greenbyte create <project_name>`
/// Creates a new project on the server and sets up local .greenbyte config.
pub async fn create(project_name: String) -> Result<(), String> {
    validate_project_name(&project_name)?;
    let server = server_url();
    validate_server_url(&server)?;
    if config_exists() {
        return Err(
            "A .greenbyte config already exists here. Use `greenbyte init` to link.".to_string(),
        );
    }

    let auth_token = crate::commands::auth::access_token(&server, None).await?;

    println!(
        "{}",
        format!("Creating project '{}'...", project_name).bold()
    );

    // Generate master key for this project — user must save this securely
    let master_key = Zeroizing::new(generate_token(32));

    println!();
    println!(
        "{}",
        "⚠  Your Master Key (save this — it cannot be recovered):"
            .yellow()
            .bold()
    );
    println!("   {}", master_key.cyan().bold());
    println!();
    println!("   This key encrypts the project's .env files on your device.");
    println!("   Every collaborator needs this key through a separate secure channel.");
    println!("   Store it in a password manager.\n");

    // Hash master key before sending to server (server never sees plaintext key)
    let master_key_hash = hash_master_key(&master_key);

    let spinner = start_spinner("Creating project on server...");

    let client = http_client()?;
    let res = client
        .post(format!("{server}/projects"))
        .bearer_auth(&auth_token)
        .json(&CreateProjectRequest {
            name: project_name.clone(),
            master_key_hash,
        })
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    spinner.finish_and_clear();

    if !res.status().is_success() {
        return Err(response_error(res, "Project creation").await);
    }

    let data: CreateProjectResponse = res
        .json()
        .await
        .map_err(|e| format!("Response parse error: {}", e))?;

    // Save local config
    save_config(&LocalConfig {
        project_name: Some(project_name.clone()),
        project_id: Some(data.project_id),
        server_url: server,
        auth_token: Some(auth_token),
        refresh_token: None,
        soft_token: Some(data.soft_token),
        refresher_token: None,
        master_key_hint: Some(format!("{}...", &master_key[..8])), // just a hint, not the key
    })?;

    println!(
        "{} Project '{}' created!",
        "✓".green().bold(),
        project_name.cyan()
    );
    println!("  .greenbyte config saved in this directory.");
    println!("  Add .greenbyte to your .gitignore!\n");

    // Auto-add to .gitignore if it exists
    append_to_gitignore()?;

    Ok(())
}

/// `greenbyte init <project_name>`
/// Links an existing project to the current directory using OTT flow.
pub async fn init(project_name: String) -> Result<(), String> {
    validate_project_name(&project_name)?;
    let server = server_url();
    validate_server_url(&server)?;
    if config_exists() {
        return Err("Already initialized. .greenbyte exists.".to_string());
    }

    println!(
        "{}",
        format!("Linking to project '{}'...", project_name).bold()
    );
    println!();

    let ott = prompt("Enter your One-Time Token (from email): ")?;
    let mac = get_device_mac()?;

    // Build refresher token = SHA256(ott + mac)
    let refresher_token = crate::utils::mac::make_refresher_token(&ott, &mac);

    let spinner = start_spinner("Verifying token...");

    let client = http_client()?;
    let res = client
        .post(format!("{server}/projects/{project_name}/join"))
        .json(&serde_json::json!({
            "ott": ott,
            "refresher_token": refresher_token,
            "mac_address": mac,
        }))
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    spinner.finish_and_clear();

    if !res.status().is_success() {
        return Err(response_error(res, "Project join").await);
    }

    let data: serde_json::Value = res
        .json()
        .await
        .map_err(|e| format!("Response error: {}", e))?;

    save_config(&LocalConfig {
        project_name: Some(project_name.clone()),
        project_id: data["project_id"].as_str().map(String::from),
        server_url: server,
        auth_token: data["auth_token"].as_str().map(String::from),
        refresh_token: data["refresh_token"].as_str().map(String::from),
        soft_token: data["soft_token"].as_str().map(String::from),
        refresher_token: Some(refresher_token),
        master_key_hint: None,
    })?;
    save_global_auth(&GlobalAuth {
        email: None,
        auth_token: data["auth_token"].as_str().map(String::from),
        user_id: None,
        refresh_token: data["refresh_token"].as_str().map(String::from),
    })?;

    println!(
        "{} Joined project '{}'!",
        "✓".green().bold(),
        project_name.cyan()
    );
    println!("  Run `greenbyte pull` to get the latest .env.\n");
    append_to_gitignore()?;
    Ok(())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn hash_master_key(master_key: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"greenbyte-project-key-v2:");
    hasher.update(master_key.as_bytes());
    hex::encode(hasher.finalize())
}

fn append_to_gitignore() -> Result<(), String> {
    let gitignore = std::path::Path::new(".gitignore");
    let mut existing = std::fs::read_to_string(gitignore).unwrap_or_default();
    let required = [
        ".greenbyte",
        ".greenbyte-logs",
        ".env",
        ".env.*",
        "!.env.example",
    ];
    let mut changed = false;
    for entry in required {
        if !existing.lines().any(|line| line.trim() == entry) {
            if !existing.is_empty() && !existing.ends_with('\n') {
                existing.push('\n');
            }
            existing.push_str(entry);
            existing.push('\n');
            changed = true;
        }
    }
    if changed {
        std::fs::write(gitignore, existing)
            .map_err(|e| format!("Could not update .gitignore: {e}"))?;
        println!("{} Added Greenbyte secret files to .gitignore", "✓".green());
    }
    Ok(())
}

fn validate_project_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 64
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(
            "Project names must be 1-64 characters using letters, numbers, '-' or '_'.".to_string(),
        );
    }
    Ok(())
}

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
