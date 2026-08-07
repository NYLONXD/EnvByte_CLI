use colored::Colorize;

use crate::utils::{
    config::{http_client, response_error},
    local_store::load_config,
};

pub async fn show() -> Result<(), String> {
    let config = load_config()?;
    let project_id = config.project_id.clone().ok_or("No project linked.")?;
    let token =
        crate::commands::auth::access_token(&config.server_url, config.auth_token.clone()).await?;
    let response = http_client()?
        .get(format!(
            "{}/projects/{project_id}/audit",
            config.server_url.trim_end_matches('/')
        ))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("Could not load audit events: {e}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Audit log").await);
    }
    let events: Vec<serde_json::Value> = response
        .json()
        .await
        .map_err(|e| format!("Invalid audit response: {e}"))?;
    println!(
        "{}",
        "─── Project Audit Log ───────────────────────────────".dimmed()
    );
    for event in events {
        println!(
            "{} | {:<28} | {:<16} | {}",
            event["created_at"].as_str().unwrap_or("unknown time"),
            event["action"].as_str().unwrap_or("unknown action"),
            event["actor"].as_str().unwrap_or("system"),
            event["metadata"],
        );
    }
    Ok(())
}
