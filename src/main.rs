use clap::{Parser, Subcommand};

/// My awesome CLI tool
#[derive(Parser)]
#[command(name = "my-cli")]
#[command(about = "A simple CLI example", long_about = None)]
struct Cli {
    /// Optional name to greet
    #[arg(short, long)]
    name: Option<String>,

    /// Turn on verbose mode
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Greet someone
    Greet {
        /// Name of the person
        name: String,
    },
    /// Show version info
    Info,
}

fn main() {
    let cli = Cli::parse();

    if cli.verbose {
        println!("Verbose mode enabled");
    }

    match &cli.command {
        Some(Commands::Greet { name }) => {
            println!("Hello, {}!", name);
        }
        Some(Commands::Info) => {
            println!("my-cli v0.1.0");
        }
        None => {
            if let Some(name) = &cli.name {
                println!("Hello, {}!", name);
            } else {
                println!("Run with --help for usage");
            }
        }
    }
}