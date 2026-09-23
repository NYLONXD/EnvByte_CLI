//! `envbyte logs` - local snapshots, or the project's server-side history.

use colored::Colorize;

use crate::{
    commands::context::ProjectContext,
    core::{api::env, workspace::commit_log},
    ui,
};

pub async fn show(output_file: Option<String>, remote: bool) -> Result<(), String> {
    if remote {
        return show_remote().await;
    }
    show_local(output_file)
}

async fn show_remote() -> Result<(), String> {
    let context = ProjectContext::load().await?;
    let commits = env::history(&context.session, &context.project_id).await?;
    if commits.is_empty() {
        ui::note("No server-side history yet.");
        return Ok(());
    }
    ui::heading(&format!("History for {}", context.config.qualified_name()));
    for commit in commits {
        println!(
            "  {} {:<20} {:<14} {:<8} {}",
            commit.commit_id[..8].cyan(),
            commit
                .created_at
                .format("%Y-%m-%d %H:%M UTC")
                .to_string()
                .dimmed(),
            commit.author,
            format!("v{}", commit.key_version).dimmed(),
            commit.message
        );
        ui::note(&format!("    {}", commit.filename));
    }
    Ok(())
}

fn show_local(output_file: Option<String>) -> Result<(), String> {
    let store = commit_log::load()?;
    if store.commits.is_empty() {
        ui::note("No local snapshots yet. Run `envbyte commit \"message\"`.");
        return Ok(());
    }
    let mut rendered = String::new();
    for commit in &store.commits {
        rendered.push_str(&format!(
            "{}  {}  {}  {}\n",
            &commit.id[..8],
            commit.timestamp.format("%Y-%m-%d %H:%M UTC"),
            commit.filename.as_deref().unwrap_or(".env"),
            commit.message
        ));
    }
    match output_file {
        // Snapshots are ciphertext, but the log still names files and times,
        // so it is written with the same care as the rest.
        Some(path) => {
            crate::core::workspace::paths::secure_atomic_write(
                std::path::Path::new(&path),
                rendered.as_bytes(),
            )?;
            ui::success(&format!(
                "Wrote {} local snapshots to {path}.",
                store.commits.len()
            ));
        }
        None => {
            ui::heading("Local snapshots");
            print!("{rendered}");
        }
    }
    Ok(())
}
