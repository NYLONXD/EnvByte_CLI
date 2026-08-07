use crate::utils::{
    config::{env_file_path, http_client, server_url},
    local_store::{config_exists, load_config, load_global_auth, load_logs},
    mac::get_device_mac,
};
use colored::Colorize;

/// `greenbyte status`
/// Shows the current state of the project — linked project, last commit, .env presence.
pub async fn show() -> Result<(), String> {
    println!(
        "{}",
        "─── Greenbyte Status ─────────────────────────────────".dimmed()
    );

    // ── Auth — fetch logged-in user from server ─────────────────────────────
    let global_auth = load_global_auth().unwrap_or_default();
    let project_config = if config_exists() {
        Some(load_config()?)
    } else {
        None
    };
    let status_server = project_config
        .as_ref()
        .map(|config| config.server_url.clone())
        .unwrap_or_else(server_url);
    let session_token = global_auth.auth_token.as_ref().or_else(|| {
        project_config
            .as_ref()
            .and_then(|config| config.auth_token.as_ref())
    });
    let has_session = session_token.is_some();
    match session_token {
        Some(token) => {
            match fetch_me(token, &status_server).await {
                Ok((username, email)) => {
                    println!(
                        "  {} Logged in as {} ({})",
                        "●".green(),
                        username.cyan().bold(),
                        email.dimmed(),
                    );
                }
                Err(e) => {
                    // Token exists but server call failed — show local email as fallback
                    eprintln!("  {} Could not verify session: {}", "⚠".yellow(), e);
                    match &global_auth.email {
                        Some(email) => println!(
                            "  {} Logged in as {} {}",
                            "●".yellow(),
                            email.cyan(),
                            "(could not reach server)".dimmed(),
                        ),
                        None => println!(
                            "  {} Logged in {}",
                            "●".yellow(),
                            "(could not verify)".dimmed()
                        ),
                    }
                }
            }
        }
        None => println!("  {} Not logged in  (run `greenbyte login`)", "●".red()),
    }

    // ── Project ─────────────────────────────────────────────────────────────
    if project_config.is_none() {
        println!("  {} No project linked in this directory", "●".yellow());
        println!(
            "{}",
            "─────────────────────────────────────────────────────".dimmed()
        );
        println!("  Run `greenbyte create <name>` or `greenbyte init <name>`");
        return Ok(());
    }

    let config = project_config.expect("project config was checked above");

    match &config.project_name {
        Some(name) => println!("  {} Project:  {}", "●".green(), name.cyan().bold()),
        None => println!("  {} Project:  {}", "●".yellow(), "unknown".dimmed()),
    }

    if let Some(id) = &config.project_id {
        println!("  {} ID:       {}", "●".green(), id.dimmed());
    }

    // ── .env file ───────────────────────────────────────────────────────────
    let env_path = env_file_path();
    if env_path.exists() {
        let metadata = std::fs::metadata(&env_path).ok();
        let size = metadata.map(|m| m.len()).unwrap_or(0);
        println!(
            "  {} .env:     {} ({} bytes)",
            "●".green(),
            "present".green(),
            size
        );
    } else {
        println!(
            "  {} .env:     {}",
            "●".yellow(),
            "not found  (run `greenbyte pull`)".yellow()
        );
    }

    // ── Device MAC ──────────────────────────────────────────────────────────
    match get_device_mac() {
        Ok(mac) => println!(
            "  {} Device:   {}...{}",
            "●".green(),
            &mac[..4],
            &mac[mac.len() - 4..]
        ),
        Err(_) => println!(
            "  {} Device:   {}",
            "●".red(),
            "MAC address unavailable".red()
        ),
    }

    // ── Token status ─────────────────────────────────────────────────────────
    let token_status = if has_session {
        "present".green()
    } else {
        "missing".red()
    };
    println!("  {} Token:    {}", "●".green(), token_status);
    let logs = load_logs().unwrap_or_default();
    let count = logs.commits.len();
    if count == 0 {
        println!("  {} Commits:  none yet", "●".dimmed());
    } else {
        let latest = logs.commits.last().unwrap();
        println!(
            "  {} Commits:  {} total  |  latest: [{}] \"{}\"",
            "●".green(),
            count,
            &latest.id[..8].cyan(),
            latest.message.dimmed()
        );
    }

    println!(
        "{}",
        "─────────────────────────────────────────────────────".dimmed()
    );
    Ok(())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

async fn fetch_me(token: &str, server: &str) -> Result<(String, String), String> {
    let client = http_client()?;
    let res = client
        .get(format!("{}/users/me", server.trim_end_matches('/')))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    if !res.status().is_success() {
        return Err(format!("Server returned {}", res.status()));
    }

    // Parse as Value so extra MongoDB fields (_id, files, owner, etc.) don't break us
    let body: serde_json::Value = res
        .json()
        .await
        .map_err(|e| format!("Parse error: {}", e))?;

    let username = body["username"].as_str().unwrap_or("unknown").to_string();
    let email = body["email"].as_str().unwrap_or("unknown").to_string();

    Ok((username, email))
}
