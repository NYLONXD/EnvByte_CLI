use colored::Colorize;
use serde::{Deserialize, Serialize};
use crate::utils::{
    config::server_url,
    local_store::{load_global_auth, save_config, config_exists, LocalConfig},
    mac::get_device_mac,
    crypto::generate_token,
};

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
    if config_exists() {
        return Err("A .greenbyte config already exists here. Use `greenbyte init` to link.".to_string());
    }

    let auth = load_global_auth()?;
    let auth_token = auth.auth_token
        .ok_or("Not logged in. Run `greenbyte login` first.")?;

    println!("{}", format!("Creating project '{}'...", project_name).bold());

    // Generate master key for this project — user must save this securely
    let master_key = generate_token(32);
    let mac = get_device_mac()?;

    println!();
    println!("{}", "⚠  Your Master Key (save this — it cannot be recovered):".yellow().bold());
    println!("   {}", master_key.cyan().bold());
    println!();
    println!("   This key + your device's MAC address encrypt your .env.");
    println!("   Store it in a password manager.\n");

    // Hash master key before sending to server (server never sees plaintext key)
    let master_key_hash = hash_master_key(&master_key, &mac);

    let spinner = start_spinner("Creating project on server...");

    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/projects", server_url()))
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
        let msg: serde_json::Value = res.json().await.unwrap_or_default();
        return Err(format!("Failed to create project: {}", msg["error"].as_str().unwrap_or("unknown")));
    }

    let data: CreateProjectResponse = res.json().await
        .map_err(|e| format!("Response parse error: {}", e))?;

    // Save local config
    save_config(&LocalConfig {
        project_name: Some(project_name.clone()),
        project_id: Some(data.project_id),
        server_url: server_url(),
        auth_token: Some(auth_token),
        soft_token: Some(data.soft_token),
        refresher_token: None,
        master_key_hint: Some(format!("{}...", &master_key[..8])), // just a hint, not the key
    })?;

    println!("{} Project '{}' created!", "✓".green().bold(), project_name.cyan());
    println!("  .greenbyte config saved in this directory.");
    println!("  Add .greenbyte to your .gitignore!\n");

    // Auto-add to .gitignore if it exists
    append_to_gitignore();

    Ok(())
}

/// `greenbyte init <project_name>`
/// Links an existing project to the current directory using OTT flow.
pub async fn init(project_name: String) -> Result<(), String> {
    if config_exists() {
        return Err("Already initialized. .greenbyte exists.".to_string());
    }

    println!("{}", format!("Linking to project '{}'...", project_name).bold());
    println!();

    let ott = prompt("Enter your One-Time Token (from email): ")?;
    let mac = get_device_mac()?;

    // Build refresher token = SHA256(ott + mac)
    let refresher_token = crate::utils::mac::make_refresher_token(&ott, &mac);

    let spinner = start_spinner("Verifying token...");

    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/projects/{}/join", server_url(), project_name))
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
        return Err("Invalid or expired token. Ask the project owner to re-add you.".to_string());
    }

    let data: serde_json::Value = res.json().await
        .map_err(|e| format!("Response error: {}", e))?;

    save_config(&LocalConfig {
        project_name: Some(project_name.clone()),
        project_id: data["project_id"].as_str().map(String::from),
        server_url: server_url(),
        auth_token: data["auth_token"].as_str().map(String::from),
        soft_token: data["soft_token"].as_str().map(String::from),
        refresher_token: Some(refresher_token),
        master_key_hint: None,
    })?;

    println!("{} Joined project '{}'!", "✓".green().bold(), project_name.cyan());
    println!("  Run `greenbyte pull` to get the latest .env.\n");
    append_to_gitignore();
    Ok(())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn hash_master_key(master_key: &str, mac: &str) -> String {
    use sha2::{Sha256, Digest};
    let input = format!("{}:{}", master_key, mac);
    hex::encode(Sha256::digest(input.as_bytes()))
}

fn append_to_gitignore() {
    let gitignore = std::path::Path::new(".gitignore");
    let entries = "\n# Greenbyte\n.greenbyte\n.greenbyte-logs\n.env\n";
    if gitignore.exists() {
        let existing = std::fs::read_to_string(gitignore).unwrap_or_default();
        if !existing.contains(".greenbyte") {
            let _ = std::fs::write(gitignore, format!("{}{}", existing, entries));
            println!("{} Added .greenbyte to .gitignore", "✓".green());
        }
    }
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