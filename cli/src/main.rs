//! Greenbyte CLI.
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

#[tokio::main]
async fn main() {
    let arguments = cli::Cli::parse();
    ui::banner();

    if let Err(error) = cli::dispatch(arguments.command).await {
        eprintln!("{} {error}", "Error:".red().bold());
        std::process::exit(1);
    }
}
