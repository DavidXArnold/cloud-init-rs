//! cloud-init-rs - A safe Rust implementation of cloud-init
//!
//! Focused on:
//! - Fast boot times
//! - Memory safety (no unsafe code)
//! - 80% compatibility with cloud-init functionality

use clap::{Parser, Subcommand};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use cloud_init_rs::{run_stages, CloudInitError, Stage};

#[derive(Parser)]
#[command(name = "cloud-init-rs")]
#[command(author, version, about = "Safe Rust implementation of cloud-init", long_about = None)]
struct Cli {
    /// Enable verbose output
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize the system (runs all stages)
    Init,
    /// Run local stage (disk setup, mounts)
    Local,
    /// Run network stage (after network is up)
    Network,
    /// Run config stage (apply configuration)
    Config,
    /// Run final stage (user scripts, etc.)
    Final,
    /// Query instance metadata
    Query {
        /// Key to query (e.g., instance-id, local-hostname)
        key: String,
    },
    /// Clean cloud-init artifacts
    Clean {
        /// Remove logs as well
        #[arg(long)]
        logs: bool,
    },
    /// Show status of cloud-init
    Status,
}

fn init_logging(verbosity: u8) {
    let level = match verbosity {
        0 => Level::INFO,
        1 => Level::DEBUG,
        _ => Level::TRACE,
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(level)
        .with_target(false)
        .with_thread_ids(false)
        .compact()
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set tracing subscriber");
}

#[tokio::main]
async fn main() -> Result<(), CloudInitError> {
    let cli = Cli::parse();
    init_logging(cli.verbose);

    match cli.command {
        Some(Commands::Init) => {
            info!("Running all cloud-init stages");
            run_stages(&[Stage::Local, Stage::Network, Stage::Config, Stage::Final]).await?;
        }
        Some(Commands::Local) => {
            info!("Running local stage");
            run_stages(&[Stage::Local]).await?;
        }
        Some(Commands::Network) => {
            info!("Running network stage");
            run_stages(&[Stage::Network]).await?;
        }
        Some(Commands::Config) => {
            info!("Running config stage");
            run_stages(&[Stage::Config]).await?;
        }
        Some(Commands::Final) => {
            info!("Running final stage");
            run_stages(&[Stage::Final]).await?;
        }
        Some(Commands::Query { key }) => {
            info!("Querying metadata key: {}", key);
            // TODO: Implement metadata query
            println!("Query not yet implemented for key: {}", key);
        }
        Some(Commands::Clean { logs }) => {
            info!("Cleaning cloud-init artifacts (logs: {})", logs);
            // TODO: Implement clean
            println!("Clean not yet implemented");
        }
        Some(Commands::Status) => {
            info!("Checking cloud-init status");
            // TODO: Implement status
            println!("Status not yet implemented");
        }
        None => {
            info!("No command specified, running init");
            run_stages(&[Stage::Local, Stage::Network, Stage::Config, Stage::Final]).await?;
        }
    }

    Ok(())
}
