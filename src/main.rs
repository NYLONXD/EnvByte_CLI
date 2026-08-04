mod commands;
mod utils;

use clap::{Parser, Subcommand};
use colored::Colorize;

#[derive(Parser)]
#[command(
    name = "greenbyte",
    about = "🌿 Team .env manager — secure, versioned, collaborative",
    version,
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

        /// Environment file to push (must be a local .env* filename)
        #[arg(short, long)]
        file: Option<String>,
    },

    /// Pull the latest .env from the server
    Pull {
        /// Select a remote .env* file without an interactive prompt
        #[arg(short, long)]
        file: Option<String>,

        /// Overwrite an existing local file without confirmation
        #[arg(long)]
        force: bool,
    },

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

        /// Show project history stored on the server
        #[arg(long)]
        remote: bool,
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

    /// List collaborators and their roles
    Members,

    /// Remove a collaborator by user ID
    Remove { user_id: String },

    /// Change a collaborator role (admin, member, or viewer)
    Role { user_id: String, role: String },

    /// Register a new Greenbyte account
    Register,

    /// Login to your Greenbyte account
    Login,

    /// Remove the locally stored login session
    Logout,

    /// Request a token and reset an account password
    ResetPassword,

    /// Show current project status
    Status,

    /// Show the server-side project security audit log
    Audit,
}

#[tokio::main]
async fn main() {
    print_banner();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Create { project_name } => commands::project::create(project_name).await,
        Commands::Init { project_name } => commands::project::init(project_name).await,
        Commands::Push { message, file } => commands::sync::push(message, file).await,
        Commands::Pull { file, force } => commands::sync::pull(file, force).await,
        Commands::Commit { message } => commands::commit::commit(message).await,
        Commands::Logs { file, remote } => commands::logs::show_logs(file, remote).await,
        Commands::Rollback { address, local } => commands::rollback::rollback(address, local).await,
        Commands::Add { email } => commands::collaborator::add(email).await,
        Commands::Members => commands::collaborator::members().await,
        Commands::Remove { user_id } => commands::collaborator::remove(user_id).await,
        Commands::Role { user_id, role } => {
            commands::collaborator::change_role(user_id, role).await
        }
        Commands::Register => commands::auth::register().await,
        Commands::Login => commands::auth::login().await,
        Commands::Logout => commands::auth::logout().await,
        Commands::ResetPassword => commands::auth::reset_password().await,
        Commands::Status => commands::status::show().await,
        Commands::Audit => commands::audit::show().await,
    };

    if let Err(e) = result {
        eprintln!("{} {}", "✗ Error:".red().bold(), e);
        std::process::exit(1);
    }
}

fn print_banner() {
    println!("{}", "🌿 Greenbyte".green().bold());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_non_interactive_push_options() {
        let cli = Cli::try_parse_from([
            "greenbyte",
            "push",
            "--file",
            ".env.ci",
            "--message",
            "deploy",
        ])
        .unwrap();
        match cli.command {
            Commands::Push { message, file } => {
                assert_eq!(message, "deploy");
                assert_eq!(file.as_deref(), Some(".env.ci"));
            }
            _ => panic!("expected push command"),
        }
    }

    #[test]
    fn parses_safe_pull_overwrite_flag() {
        let cli =
            Cli::try_parse_from(["greenbyte", "pull", "--file", ".env.ci", "--force"]).unwrap();
        match cli.command {
            Commands::Pull { file, force } => {
                assert_eq!(file.as_deref(), Some(".env.ci"));
                assert!(force);
            }
            _ => panic!("expected pull command"),
        }
    }
}
