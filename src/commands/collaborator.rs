use crate::utils::{
    config::{http_client, response_error},
    local_store::load_config,
};
use colored::Colorize;

/// `greenbyte add <email>`
/// Adds a collaborator to the current project.
/// Server generates an OTT and sends it to the collaborator's email.
/// They use it with `greenbyte init <project>` to join.
pub async fn add(email: String) -> Result<(), String> {
    if !email.contains('@') || email.chars().any(char::is_whitespace) {
        return Err("Enter a valid collaborator email address.".to_string());
    }
    let config = load_config()?;

    let project_id = config
        .project_id
        .ok_or("No project linked. Run `greenbyte create` or `greenbyte init`.")?;

    let project_name = config
        .project_name
        .unwrap_or_else(|| "this project".to_string());

    let auth_token =
        crate::commands::auth::access_token(&config.server_url, config.auth_token.clone()).await?;

    println!(
        "{}",
        format!("Adding {} to '{}'...", email, project_name).bold()
    );

    let spinner = start_spinner("Generating one-time token...");

    let client = http_client()?;
    let res = client
        .post(format!(
            "{}/projects/{}/collaborators",
            config.server_url.trim_end_matches('/'),
            project_id
        ))
        .bearer_auth(&auth_token)
        .json(&serde_json::json!({ "email": email }))
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    spinner.finish_and_clear();

    if !res.status().is_success() {
        return Err(response_error(res, "Collaborator invite").await);
    }
    let body: serde_json::Value = res.json().await.unwrap_or_default();
    let expires = body["expires_in_hours"].as_i64().unwrap_or(24);

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
    println!("  {} Token expires in {expires} hours.", "ℹ".blue());

    Ok(())
}

pub async fn members() -> Result<(), String> {
    let (config, project_id, token) = project_session().await?;
    let response = http_client()?
        .get(format!(
            "{}/projects/{project_id}/collaborators",
            config.server_url.trim_end_matches('/')
        ))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("Could not load collaborators: {e}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Collaborator list").await);
    }
    let members: Vec<serde_json::Value> = response
        .json()
        .await
        .map_err(|e| format!("Invalid collaborator response: {e}"))?;
    println!(
        "{}",
        "─── Project Members ─────────────────────────────────".dimmed()
    );
    for member in members {
        println!(
            "  {:<16} {:<24} {:<8} {}",
            member["username"].as_str().unwrap_or("unknown"),
            member["email"].as_str().unwrap_or("unknown"),
            member["role"].as_str().unwrap_or("unknown"),
            member["user_id"].as_str().unwrap_or("unknown").dimmed(),
        );
    }
    Ok(())
}

pub async fn remove(user_id: String) -> Result<(), String> {
    uuid::Uuid::parse_str(&user_id).map_err(|_| "Invalid collaborator user ID.".to_string())?;
    let confirmation = prompt(&format!("Remove collaborator {user_id}? [y/N]: "))?;
    if confirmation.to_lowercase() != "y" {
        println!("Aborted.");
        return Ok(());
    }
    let (config, project_id, token) = project_session().await?;
    let response = http_client()?
        .delete(format!(
            "{}/projects/{project_id}/collaborators/{user_id}",
            config.server_url.trim_end_matches('/')
        ))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("Could not remove collaborator: {e}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Collaborator removal").await);
    }
    println!("{} Collaborator removed.", "✓".green().bold());
    Ok(())
}

pub async fn change_role(user_id: String, role: String) -> Result<(), String> {
    uuid::Uuid::parse_str(&user_id).map_err(|_| "Invalid collaborator user ID.".to_string())?;
    if !matches!(role.as_str(), "admin" | "member" | "viewer") {
        return Err("Role must be admin, member, or viewer.".to_string());
    }
    let (config, project_id, token) = project_session().await?;
    let response = http_client()?
        .patch(format!(
            "{}/projects/{project_id}/collaborators/{user_id}",
            config.server_url.trim_end_matches('/')
        ))
        .bearer_auth(token)
        .json(&serde_json::json!({ "role": role }))
        .send()
        .await
        .map_err(|e| format!("Could not change collaborator role: {e}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Role change").await);
    }
    println!(
        "{} Collaborator role updated to {}.",
        "✓".green().bold(),
        role.cyan()
    );
    Ok(())
}

async fn project_session(
) -> Result<(crate::utils::local_store::LocalConfig, String, String), String> {
    let config = load_config()?;
    let project_id = config.project_id.clone().ok_or("No project linked.")?;
    let token =
        crate::commands::auth::access_token(&config.server_url, config.auth_token.clone()).await?;
    Ok((config, project_id, token))
}

fn prompt(label: &str) -> Result<String, String> {
    use std::io::{self, Write};
    print!("{label}");
    io::stdout().flush().map_err(|e| e.to_string())?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| e.to_string())?;
    Ok(input.trim().to_string())
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
