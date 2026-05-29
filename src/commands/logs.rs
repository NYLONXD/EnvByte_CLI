use colored::Colorize;
use crate::utils::local_store::load_logs;

/// `greenbyte logs [--file <filename>]`
/// Shows local commit history. Optionally writes to a file.
pub async fn show_logs(file: Option<String>) -> Result<(), String> {
    let store = load_logs()?;

    if store.commits.is_empty() {
        println!("{}", "No commits yet. Use `greenbyte commit` or `greenbyte push`.".dimmed());
        return Ok(());
    }

    let total = store.commits.len();
    let mut output = String::new();
    output.push_str("─── Greenbyte Logs ──────────────────────────────────\n");

    for (i, commit) in store.commits.iter().enumerate().rev() {
        let tag = if i == total - 1 { "latest" } else { "      " };
        let line = format!(
            "[{}] {} | {} | \"{}\"\n",
            &commit.id[..8],
            commit.timestamp.format("%Y-%m-%d %H:%M UTC"),
            tag,
            commit.message,
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