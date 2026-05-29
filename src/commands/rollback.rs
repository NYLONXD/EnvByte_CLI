use colored::Colorize;
use crate::utils::{
    config::{server_url, write_env_file},
    local_store::{load_config, load_logs, load_global_auth},
    crypto::{decrypt_env, EncryptedPayload},
    mac::get_device_mac,
};

/// `greenbyte rollback --address <id>`  → rollback on server to a remote commit
/// `greenbyte rollback --local <id>`    → restore local .env from a local commit snapshot
pub async fn rollback(address: Option<String>, local: Option<String>) -> Result<(), String> {
    match (address, local) {
        (Some(addr), None) => rollback_server(addr).await,
        (None, Some(local_id)) => rollback_local(local_id).await,
        (Some(_), Some(_)) => Err("Use either --address OR --local, not both.".to_string()),
        (None, None) => Err(
            "Provide --address <id> for server rollback or --local <id> for local rollback."
                .to_string(),
        ),
    }
}

// ─── Server rollback ──────────────────────────────────────────────────────────

async fn rollback_server(address: String) -> Result<(), String> {
    let config = load_config()?;

    // Clone before ok_or consumes the Option, so config is still usable after
    let project_id = config.project_id.clone()
        .ok_or("No project linked.")?;

    let auth = load_global_auth()?;
    let auth_token = config.auth_token
        .or(auth.auth_token)
        .ok_or("Not logged in. Run `greenbyte login`.")?;

    let short = &address[..8.min(address.len())];
    println!("{}", format!("Rolling back to server commit {}...", short).bold());

    let confirm = prompt("This will overwrite the current server .env. Continue? [y/N]: ")?;
    if confirm.to_lowercase() != "y" {
        println!("Aborted.");
        return Ok(());
    }

    let spinner = start_spinner("Rolling back on server...");

    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/projects/{}/rollback", server_url(), project_id))
        .bearer_auth(&auth_token)
        .json(&serde_json::json!({ "commit_id": address }))
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    spinner.finish_and_clear();

    if !res.status().is_success() {
        let msg: serde_json::Value = res.json().await.unwrap_or_default();
        return Err(format!(
            "Rollback failed: {}",
            msg["error"].as_str().unwrap_or("unknown")
        ));
    }

    println!(
        "{} Server rolled back to commit {}",
        "✓".green().bold(),
        short.cyan()
    );
    println!("  Run `greenbyte pull` to sync your local .env.");
    Ok(())
}

// ─── Local rollback ───────────────────────────────────────────────────────────

async fn rollback_local(local_id: String) -> Result<(), String> {
    let store = load_logs()?;

    // Match by full ID or by short prefix (first 8 chars)
    let commit = store
        .commits
        .iter()
        .find(|c| c.id == local_id || c.id.starts_with(&local_id))
        .ok_or_else(|| {
            format!("No local commit found with ID starting with '{}'", local_id)
        })?;

    println!(
        "{}",
        format!("Restoring local .env from commit '{}'", &commit.id[..8]).bold()
    );
    println!("  Message:   \"{}\"", commit.message);
    println!("  Timestamp: {}", commit.timestamp.format("%Y-%m-%d %H:%M UTC"));
    println!();

    let confirm = prompt("This will overwrite your local .env. Continue? [y/N]: ")?;
    if confirm.to_lowercase() != "y" {
        println!("Aborted.");
        return Ok(());
    }

    let mac = get_device_mac()?;
    let master_key = rpassword::prompt_password("Master Key: ").map_err(|e| e.to_string())?;

    let payload = EncryptedPayload {
        data: commit.env_snapshot.clone(),
    };
    let decrypted = decrypt_env(&payload, &master_key, &mac)?;

    write_env_file(&decrypted)?;

    println!(
        "{} .env restored from local commit {}",
        "✓".green().bold(),
        commit.id[..8].cyan()
    );
    Ok(())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

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