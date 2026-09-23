//! The command surface.
//!
//! Kept separate from `main` so that adding a command is one variant here and
//! one arm in `dispatch`, with the work itself living in `commands`.

use clap::{Parser, Subcommand};

use crate::commands;

#[derive(Parser)]
#[command(
    name = "envbyte",
    about = "Team .env manager - encrypted, versioned, collaborative",
    version,
    long_about = "Envbyte shares .env files across a team without anyone sending a secret to \
                  anyone. Each member holds an identity key; the project key is sealed to it."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a new project in this directory
    Create {
        /// Name of the project, unique within your own account
        project_name: String,
    },

    /// Link this directory to a project you were invited to
    Init {
        /// Optional: the project you expect the invitation to be for
        project_name: Option<String>,
    },

    /// List the projects you can reach
    Projects,

    /// Encrypt the local .env and push it
    Push {
        #[arg(short, long, default_value = "update")]
        message: String,
        /// Push a specific .env* file without being asked
        #[arg(short, long)]
        file: Option<String>,
    },

    /// Fetch and decrypt the project's .env
    Pull {
        /// Pull a specific .env* file without being asked
        #[arg(short, long)]
        file: Option<String>,
        /// Overwrite an existing local file without confirmation
        #[arg(long)]
        force: bool,
    },

    /// Take an encrypted local snapshot of the current .env
    Commit { message: String },

    /// Show snapshots, or the project's server-side history
    Logs {
        /// Write the list to a file instead of printing it
        #[arg(long)]
        file: Option<String>,
        /// Show the project's server-side history
        #[arg(long)]
        remote: bool,
    },

    /// Restore a previous state
    Rollback {
        /// Roll the server back to this commit
        #[arg(long)]
        address: Option<String>,
        /// Restore the local .env from this local snapshot
        #[arg(long)]
        local: Option<String>,
    },

    /// Invite a collaborator, sealing the project key to them
    Add { email: String },

    /// List collaborators, their roles and which project key they hold
    Members,

    /// Remove a collaborator
    Remove { user_id: String },

    /// Change a collaborator's role (admin, member or viewer)
    Role { user_id: String, role: String },

    /// Retire the project key and issue a new one to current members
    Rotate {
        /// Skip the confirmation prompt, for scripted use
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Inspect or manage this device's identity key
    Identity {
        #[command(subcommand)]
        action: Option<IdentityAction>,
    },

    /// Create an account
    Register,

    /// Resend the email verification token and finish signing up
    Verify,

    /// Sign in
    Login,

    /// Sign out on this machine
    Logout,

    /// Show the signed-in account
    Whoami,

    /// Request a token and set a new password
    ResetPassword,

    /// Show what this directory is linked to
    Status,

    /// Show the project's security audit log
    Audit,
}

#[derive(Subcommand)]
pub enum IdentityAction {
    /// Show this device's identity key (the default)
    Show,
    /// Publish this device's public key to your account
    Publish,
    /// Print the secret key, for another machine or CI
    Export,
    /// Replace this device's identity key
    Replace,
}

pub async fn dispatch(command: Commands) -> Result<(), String> {
    match command {
        Commands::Create { project_name } => commands::project::create(project_name).await,
        Commands::Init { project_name } => commands::project::init(project_name).await,
        Commands::Projects => commands::project::list().await,
        Commands::Push { message, file } => commands::sync::push(message, file).await,
        Commands::Pull { file, force } => commands::sync::pull(file, force).await,
        Commands::Commit { message } => commands::snapshot::commit(message).await,
        Commands::Logs { file, remote } => commands::history::show(file, remote).await,
        Commands::Rollback { address, local } => commands::rollback::rollback(address, local).await,
        Commands::Add { email } => commands::members::add(email).await,
        Commands::Members => commands::members::list().await,
        Commands::Remove { user_id } => commands::members::remove(user_id).await,
        Commands::Role { user_id, role } => commands::members::change_role(user_id, role).await,
        Commands::Rotate { yes } => commands::rotate::rotate(yes).await,
        Commands::Identity { action } => match action.unwrap_or(IdentityAction::Show) {
            IdentityAction::Show => commands::identity::show().await,
            IdentityAction::Publish => commands::identity::publish().await,
            IdentityAction::Export => commands::identity::export().await,
            IdentityAction::Replace => commands::identity::replace().await,
        },
        Commands::Register => commands::account::register().await,
        Commands::Verify => commands::account::verify().await,
        Commands::Login => commands::account::login().await,
        Commands::Logout => commands::account::logout().await,
        Commands::Whoami => commands::account::whoami().await,
        Commands::ResetPassword => commands::account::reset_password().await,
        Commands::Status => commands::status::show().await,
        Commands::Audit => commands::audit::show().await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn the_command_surface_is_well_formed() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_non_interactive_push_options() {
        let cli = Cli::try_parse_from([
            "envbyte",
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
            _ => panic!("expected push"),
        }
    }

    #[test]
    fn parses_the_safe_pull_overwrite_flag() {
        let cli = Cli::try_parse_from(["envbyte", "pull", "--file", ".env.ci", "--force"]).unwrap();
        match cli.command {
            Commands::Pull { file, force } => {
                assert_eq!(file.as_deref(), Some(".env.ci"));
                assert!(force);
            }
            _ => panic!("expected pull"),
        }
    }

    #[test]
    fn rotation_can_be_scripted() {
        let cli = Cli::try_parse_from(["envbyte", "rotate", "-y"]).unwrap();
        assert!(matches!(cli.command, Commands::Rotate { yes: true }));
        let cli = Cli::try_parse_from(["envbyte", "rotate"]).unwrap();
        assert!(matches!(cli.command, Commands::Rotate { yes: false }));
    }

    #[test]
    fn init_no_longer_requires_a_project_name() {
        // The invitation identifies the project, so the name is optional and
        // never has to be guessed.
        let cli = Cli::try_parse_from(["envbyte", "init"]).unwrap();
        assert!(matches!(cli.command, Commands::Init { project_name: None }));
    }

    #[test]
    fn identity_defaults_to_showing() {
        let cli = Cli::try_parse_from(["envbyte", "identity"]).unwrap();
        assert!(matches!(cli.command, Commands::Identity { action: None }));
    }
}
