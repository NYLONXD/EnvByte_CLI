use crate::shared::{
    crypto::{generate_token, master_key_verifier},
    device::get_device_mac,
    http::{http_client, response_error, server_url, validate_server_url},
    storage::{config_exists, save_config, LocalConfig},
    terminal::{prompt, start_spinner},
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

    let auth_token = crate::commands::account::access_token(&server, None).await?;

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

    // Send only a verifier for the key, never the key itself.
    let master_key_hash = master_key_verifier(&master_key);

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

    // An invitation proves you were invited, not who you are. Joining happens
    // as the signed-in account, so a leaked token cannot become a session.
    let auth_token = crate::commands::account::access_token(&server, None).await?;

    let ott = prompt("Enter your One-Time Token (from email): ")?;
    let mac = get_device_mac();

    // Build refresher token = SHA256(ott + mac)
    let refresher_token = crate::shared::device::make_refresher_token(&ott, mac.as_deref());

    let spinner = start_spinner("Verifying token...");

    let client = http_client()?;
    let res = client
        .post(format!("{server}/projects/{project_name}/join"))
        .bearer_auth(&auth_token)
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
        auth_token: Some(auth_token),
        refresh_token: None,
        soft_token: data["soft_token"].as_str().map(String::from),
        refresher_token: Some(refresher_token),
        master_key_hint: None,
    })?;

    println!(
        "{} Joined project '{}'!",
        "✓".green().bold(),
        project_name.cyan()
    );
    println!("  Ask the project owner for the master key over a secure channel,");
    println!("  then run `greenbyte pull` to get the latest .env.\n");
    append_to_gitignore()?;
    Ok(())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

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
