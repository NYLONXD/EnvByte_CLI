use crate::utils::{
    config::read_env_file,
    crypto::{encrypt_env, read_master_key},
    local_store::{append_commit, load_config, LocalCommit},
};
use colored::Colorize;
use uuid::Uuid;

/// `greenbyte commit "message"`
/// Takes a local snapshot of the current .env — like a local git commit.
/// Encrypted snapshot is stored in .greenbyte-logs.
pub async fn commit(message: String) -> Result<(), String> {
    let env_content = read_env_file()?;
    let config = load_config()?;
    let master_key = read_master_key()?;

    let encrypted = encrypt_env(&env_content, &master_key)?;
    let id = Uuid::new_v4().to_string();

    let commit = LocalCommit {
        id: id.clone(),
        message: message.clone(),
        timestamp: chrono::Utc::now(),
        env_snapshot: encrypted.data,
        filename: Some(".env".to_string()),
        project_id: config.project_id,
        remote_commit_id: None,
    };

    append_commit(commit)?;

    println!("{} Local commit saved", "✓".green().bold());
    println!("  ID:      {}", id.cyan());
    println!("  Message: \"{}\"", message);
    println!(
        "  Use `greenbyte rollback --local {}` to restore.",
        &id[..8]
    );
    Ok(())
}
