//! Terminal input and output.
//!
//! Every prompt, spinner and status line lives here, so the command modules
//! stay about what they do rather than how it looks, and so output style stays
//! consistent across the CLI.

use colored::Colorize;

pub fn banner() {
    println!("{}", "Greenbyte".green().bold());
}

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

/// Reads a secret without echoing it.
pub fn prompt_password(label: &str) -> Result<String, String> {
    rpassword::prompt_password(label).map_err(|e| e.to_string())
}

/// Asks a yes/no question, defaulting to no. Destructive actions use this.
pub fn confirm(question: &str) -> Result<bool, String> {
    Ok(prompt(&format!("{question} [y/N]: "))?.to_lowercase() == "y")
}

/// Offers a numbered choice, skipping the prompt when there is only one.
pub fn choose<T>(
    title: &str,
    options: &[T],
    label: impl Fn(&T) -> String,
) -> Result<usize, String> {
    match options.len() {
        0 => Err("Nothing to choose from.".to_string()),
        1 => Ok(0),
        _ => {
            println!("{}", title.bold());
            for (index, option) in options.iter().enumerate() {
                println!("  [{}] {}", (index + 1).to_string().cyan(), label(option));
            }
            let choice = prompt(&format!("Select (1-{}): ", options.len()))?;
            let index = choice
                .parse::<usize>()
                .map_err(|_| "Invalid selection.".to_string())?
                .checked_sub(1)
                .ok_or("Invalid selection.")?;
            if index >= options.len() {
                return Err("Selection out of range.".to_string());
            }
            Ok(index)
        }
    }
}

pub fn spinner(message: &str) -> indicatif::ProgressBar {
    use indicatif::{ProgressBar, ProgressStyle};
    let bar = ProgressBar::new_spinner();
    bar.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
    );
    bar.set_message(message.to_string());
    bar.enable_steady_tick(std::time::Duration::from_millis(80));
    bar
}

pub fn success(message: &str) {
    println!("{} {message}", "OK".green().bold());
}

pub fn step(message: &str) {
    println!("  {} {message}", "-".green());
}

pub fn warn(message: &str) {
    eprintln!("{} {message}", "!".yellow().bold());
}

pub fn note(message: &str) {
    println!("  {}", message.dimmed());
}

pub fn heading(title: &str) {
    println!("{}", format!("--- {title} ").dimmed());
}

pub fn field(label: &str, value: &str) {
    println!("  {:<10} {value}", format!("{label}:"));
}
