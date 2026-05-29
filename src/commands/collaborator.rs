use colored::Colorize;
use crate::utils::{
    config::server_url,
    local_store::{load_config, load_global_auth},
};

/// `greenbyte add <email>`
/// Adds a collaborator to the current project.
/// Server generates an OTT and sends it to the collaborator's email.
/// They use it with `greenbyte init <project>` to join.
pub async fn add(email: String) -> Result<(), String> {
    let config = load_config()?;

    let project_id = config.project_id
        .ok_or("No project linked. Run `greenbyte create` or `greenbyte init`.")?;

    let project_name = config.project_name
        .unwrap_or_else(|| "this project".to_string());

    let auth = load_global_auth()?;
    let auth_token = config.auth_token
        .or(auth.auth_token)
        .ok_or("Not logged in. Run `greenbyte login`.")?;

    println!("{}", format!("Adding {} to '{}'...", email, project_name).bold());

    let spinner = start_spinner("Generating one-time token...");

    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/projects/{}/collaborators", server_url(), project_id))
        .bearer_auth(&auth_token)
        .json(&serde_json::json!({ "email": email }))
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    spinner.finish_and_clear();

    if !res.status().is_success() {
        let msg: serde_json::Value = res.json().await.unwrap_or_default();
        return Err(format!(
            "Failed to add collaborator: {}",
            msg["error"].as_str().unwrap_or("unknown")
        ));
    }

    println!("{} Invite sent to {}", "✓".green().bold(), email.cyan());
    println!();
    println!("  They will receive an email with a One-Time Token.");
    println!("  Tell them to run:");
    println!(
        "    {}",
        format!("greenbyte init {}", project_name).yellow().bold()
    );
    println!("  ...and enter the token from their email when prompted.");
    println!();
    println!("  {} Token expires in 24 hours.", "ℹ".blue());

    Ok(())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

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