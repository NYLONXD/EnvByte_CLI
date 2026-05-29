mod commands;
mod utils;

use clap::{Parser, Subcommand};
use colored::Colorize;

#[derive(Parser)]
#[command(
    name = "greenbyte",
    about = "🌿 Team .env manager — secure, versioned, collaborative",
    version = "0.1.0",
    long_about = "Greenbyte lets your team share and sync .env files securely.\nNo more WhatsApp. No more leaks."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new Greenbyte project
    Create {
        /// Name of the project
        project_name: String,
    },

    /// Initialize Greenbyte in the current directory (link to existing project)
    Init {
        /// Project name to link to
        project_name: String,
    },

    /// Push your local .env to the server
    Push {
        /// Optional commit message
        #[arg(short, long, default_value = "update")]
        message: String,
    },

    /// Pull the latest .env from the server
    Pull,

    /// Commit the current .env state with a message (local snapshot)
    Commit {
        /// Commit message
        message: String,
    },

    /// Show logs of all pushes and commits
    Logs {
        /// Save logs to a file instead of printing
        #[arg(long)]
        file: Option<String>,
    },

    /// Rollback to a previous state
    Rollback {
        /// Address (commit ID) to rollback to on the server
        #[arg(long)]
        address: Option<String>,

        /// Address (commit ID) to rollback to locally
        #[arg(long)]
        local: Option<String>,
    },

    /// Add a collaborator to the project by email
    Add {
        /// Collaborator's email address
        email: String,
    },

    /// Register a new Greenbyte account
    Register,

    /// Login to your Greenbyte account
    Login,

    /// Show current project status
    Status,
}

#[tokio::main]
async fn main() {
    print_banner();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Create { project_name } => {
            commands::project::create(project_name).await
        }
        Commands::Init { project_name } => {
            commands::project::init(project_name).await
        }
        Commands::Push { message } => {
            commands::sync::push(message).await
        }
        Commands::Pull => {
            commands::sync::pull().await
        }
        Commands::Commit { message } => {
            commands::commit::commit(message).await
        }
        Commands::Logs { file } => {
            commands::logs::show_logs(file).await
        }
        Commands::Rollback { address, local } => {
            commands::rollback::rollback(address, local).await
        }
        Commands::Add { email } => {
            commands::collaborator::add(email).await
        }
        Commands::Register => {
            commands::auth::register().await
        }
        Commands::Login => {
            commands::auth::login().await
        }
        Commands::Status => {
            commands::status::show().await
        }
    };

    if let Err(e) = result {
        eprintln!("{} {}", "✗ Error:".red().bold(), e);
        std::process::exit(1);
    }
}

fn print_banner() {
    println!("{}", "🌿 Greenbyte".green().bold());
}