use crate::shared::{
    http::{http_client, response_error},
    storage::{load_config, load_logs},
};
use colored::Colorize;

/// `greenbyte logs [--file <filename>]`
/// Shows local commit history. Optionally writes to a file.
pub async fn show_logs(file: Option<String>, remote: bool) -> Result<(), String> {
    if remote {
        return show_remote_logs(file).await;
    }
    let store = load_logs()?;

    if store.commits.is_empty() {
        println!(
            "{}",
            "No commits yet. Use `greenbyte commit` or `greenbyte push`.".dimmed()
        );
        return Ok(());
    }

    let total = store.commits.len();
    let mut output = String::new();
    output.push_str("─── Greenbyte Logs ──────────────────────────────────\n");

    for (i, commit) in store.commits.iter().enumerate().rev() {
        let tag = if i == total - 1 { "latest" } else { "      " };
        let line = format!(
            "[{}] {} | {} | {} | \"{}\"{}\n",
            &commit.id[..8],
            commit.timestamp.format("%Y-%m-%d %H:%M UTC"),
            tag,
            commit.filename.as_deref().unwrap_or(".env"),
            commit.message,
            commit
                .remote_commit_id
                .as_ref()
                .map(|id| format!(" | remote: {}", &id[..8.min(id.len())]))
                .unwrap_or_default(),
        );
        output.push_str(&line);
    }

    output.push_str("─────────────────────────────────────────────────────\n");

    match file {
        Some(filename) => {
            std::fs::write(&filename, &output)
                .map_err(|e| format!("Could not write log file: {}", e))?;
            println!("{} Logs saved to {}", "✓".green().bold(), filename.cyan());
        }
        None => {
            print!("{}", output);
        }
    }

    Ok(())
}

async fn show_remote_logs(file: Option<String>) -> Result<(), String> {
    let config = load_config()?;
    let project_id = config.project_id.clone().ok_or("No project linked.")?;
    let token =
        crate::commands::account::access_token(&config.server_url, config.auth_token.clone())
            .await?;
    let response = http_client()?
        .get(format!(
            "{}/projects/{project_id}/commits",
            config.server_url.trim_end_matches('/')
        ))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("Could not load remote history: {e}"))?;
    if !response.status().is_success() {
        return Err(response_error(response, "Remote history").await);
    }
    let commits: Vec<serde_json::Value> = response
        .json()
        .await
        .map_err(|e| format!("Invalid remote history response: {e}"))?;
    let mut output = String::from("─── Greenbyte Remote Logs ───────────────────────────\n");
    for commit in commits {
        let id = commit["commit_id"].as_str().unwrap_or("unknown");
        output.push_str(&format!(
            "[{}] {} | {} | {} | \"{}\"\n",
            &id[..8.min(id.len())],
            commit["created_at"].as_str().unwrap_or("unknown time"),
            commit["author"].as_str().unwrap_or("unknown author"),
            commit["filename"].as_str().unwrap_or(".env"),
            commit["message"].as_str().unwrap_or_default(),
        ));
    }
    output.push_str("─────────────────────────────────────────────────────\n");
    write_or_print(file, &output)
}

fn write_or_print(file: Option<String>, output: &str) -> Result<(), String> {
    if let Some(filename) = file {
        std::fs::write(&filename, output).map_err(|e| format!("Could not write log file: {e}"))?;
        println!("{} Logs saved to {}", "✓".green().bold(), filename.cyan());
    } else {
        print!("{output}");
    }
    Ok(())
}
