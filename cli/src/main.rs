//! Envbyte CLI.
//!
//! Layering, outermost first:
//!   `cli`      - the command surface and dispatch
//!   `commands` - one module per command group, thin over `core`
//!   `ui`       - every prompt, spinner and line of output
//!   `core`     - crypto, the API client and on-disk state; no terminal I/O
//!
//! Commands may use `core` and `ui`. Nothing in `core` reaches back up.

mod cli;
mod commands;
mod core;
mod ui;

use clap::Parser;
use colored::Colorize;

use crate::core::workspace::legacy;

#[tokio::main]
async fn main() {
    let arguments = cli::Cli::parse();
    ui::banner();
    carry_over_pre_rename_files();

    if let Err(error) = cli::dispatch(arguments.command).await {
        eprintln!("{} {error}", "Error:".red().bold());
        std::process::exit(1);
    }
}

/// Greenbyte became Envbyte; carry its files over so nobody loses their
/// identity key or project link to the rename.
fn carry_over_pre_rename_files() {
    let (carried, failures) = legacy::migrate();
    for entry in &carried {
        let verb = match entry.transfer {
            legacy::Transfer::Copy => "Copied",
            legacy::Transfer::Move => "Moved",
        };
        ui::step(&format!(
            "{verb} {} to {} (Greenbyte is now Envbyte)",
            entry.from.display(),
            entry.to.display()
        ));
    }
    if legacy::carried_project_link(&carried) {
        if let Err(error) = commands::project::add_to_gitignore() {
            ui::warn(&error);
        }
    }
    for failure in failures {
        ui::warn(&failure);
    }
}
