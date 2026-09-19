//! Interactive terminal helpers shared by the commands: prompts and spinners.

pub fn prompt(label: &str) -> Result<String, String> {
    use std::io::{self, Write};
    print!("{label}");
    io::stdout().flush().map_err(|e| e.to_string())?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| e.to_string())?;
    Ok(input.trim().to_string())
}

/// Reads a secret without echoing it to the terminal.
pub fn prompt_password(label: &str) -> Result<String, String> {
    rpassword::prompt_password(label).map_err(|e| e.to_string())
}

pub fn start_spinner(msg: &str) -> indicatif::ProgressBar {
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
