//! Everything Setup prints or asks.
//!
//! The symbols (√ × !) and the progress bar characters all exist in Consolas,
//! the font of the classic console window on Windows 10, so nothing renders as
//! a box there.

use std::io::{self, BufRead, Write};
use std::path::Path;
use std::time::Duration;

use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

use crate::setup::Failure;
use crate::SITE;

/// Colour needs the console's ANSI mode, which is off by default in the
/// classic console window. Where it cannot be turned on, print plain text.
pub fn init() {
    if colored::control::set_virtual_terminal(true).is_err() {
        colored::control::set_override(false);
    }
}

pub fn title() {
    println!();
    println!("  {}", "Envbyte Setup".green().bold());
    println!("  {}", "Share .env files. Never the secrets.".dimmed());
    println!();
}

pub fn help() {
    println!(
        "Envbyte Setup {}
Installs the Envbyte command-line tool for the current user.

Usage: envbyte-setup [--yes]

  -y, --yes    install without asking, adding Envbyte to PATH
  -h, --help   show this help

Settings (environment variables):
  ENVBYTE_VERSION=0.4.0      install that version instead of the latest
  ENVBYTE_INSTALL_DIR=<dir>  install somewhere other than %USERPROFILE%\\.envbyte\\bin
  ENVBYTE_NO_MODIFY_PATH=1   leave PATH alone",
        env!("CARGO_PKG_VERSION")
    );
}

pub fn blank() {
    println!();
}

pub fn field(label: &str, value: &str) {
    println!("  {} {value}", format!("{label:<11}").bold());
}

pub fn note(text: &str) {
    println!("  {}", text.dimmed());
}

pub fn done(text: &str) {
    println!("  {} {text}", "√".green().bold());
}

pub fn warn(text: &str) {
    println!("  {} {text}", "!".yellow().bold());
}

pub fn fail(failure: &Failure) {
    eprintln!();
    eprintln!("  {} {}", "×".red().bold(), failure.message.red().bold());
    if let Some(cause) = &failure.cause {
        eprintln!("    {cause}");
    }
    if let Some(hint) = &failure.hint {
        eprintln!();
        for line in hint.lines() {
            eprintln!("    {line}");
        }
    }
}

/// Asks a question whose default answer is yes. With `assume_yes` it answers
/// by itself; if input ends before an answer, the answer is no, so a closed or
/// piped stdin never installs anything by accident.
pub fn confirm(question: &str, assume_yes: bool) -> bool {
    let prompt = format!("  {} {} ", question.bold(), "[Y/n]".dimmed());
    if assume_yes {
        println!("{prompt}y");
        return true;
    }
    let stdin = io::stdin();
    loop {
        print!("{prompt}");
        let _ = io::stdout().flush();
        let mut answer = String::new();
        match stdin.lock().read_line(&mut answer) {
            Ok(0) | Err(_) => {
                println!();
                return false;
            }
            Ok(_) => {}
        }
        match answer.trim().to_ascii_lowercase().as_str() {
            "" | "y" | "yes" => return true,
            "n" | "no" => return false,
            _ => note("Please answer y or n."),
        }
    }
}

pub fn spinner(message: &str) -> ProgressBar {
    let bar = ProgressBar::new_spinner();
    bar.set_style(
        ProgressStyle::with_template("  {spinner:.green} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
            .tick_chars("-\\|/ "),
    );
    bar.set_message(message.to_string());
    bar.enable_steady_tick(Duration::from_millis(100));
    bar
}

/// A byte counter for a download, with a bar when the size is known up front.
pub fn download_bar(total: Option<u64>) -> ProgressBar {
    let (bar, template) = match total {
        Some(total) => (
            ProgressBar::new(total),
            "  Downloading [{bar:30.green/dim}] {bytes}/{total_bytes} · {bytes_per_sec} · {eta}",
        ),
        None => (
            ProgressBar::new_spinner(),
            "  {spinner:.green} Downloading {bytes} · {bytes_per_sec}",
        ),
    };
    bar.set_style(
        ProgressStyle::with_template(template)
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("█▓░")
            .tick_chars("-\\|/ "),
    );
    bar
}

pub enum PathState {
    /// Setup just added the folder, so only terminals opened from now on see it.
    Added,
    AlreadyThere,
    Missing,
}

pub fn installed(path: PathState, exe: &Path) {
    let dir = exe.parent().unwrap_or(exe).display();
    println!();
    println!(
        "  {} {}",
        "√".green().bold(),
        "Envbyte has been installed successfully.".green().bold()
    );
    println!();
    match path {
        PathState::Added => {
            println!("  You can now run: {}", "envbyte".cyan().bold());
            note("Open a new terminal window first, so it picks up the updated PATH.");
        }
        PathState::AlreadyThere => {
            println!("  You can now run: {}", "envbyte".cyan().bold());
        }
        PathState::Missing => {
            println!(
                "  You can now run: {}",
                exe.display().to_string().cyan().bold()
            );
            note(&format!(
                "Add {dir} to your PATH to run it as plain `envbyte` from any folder."
            ));
        }
    }
    println!();
    println!("  Get started:");
    println!("    {}  create an account", "envbyte register   ".cyan());
    println!(
        "    {}  start a project in this directory",
        "envbyte create app ".cyan()
    );
    println!("    {}  list every command", "envbyte --help     ".cyan());
    println!();
    println!("  Docs: {SITE}");
}

/// Holds the window open. Only used when Windows opened the console for Setup
/// (it was double-clicked), which would otherwise close the moment Setup ends,
/// taking the result with it.
pub fn pause() {
    print!("\n  Press Enter to close this window.");
    let _ = io::stdout().flush();
    let _ = io::stdin().lock().read_line(&mut String::new());
}
