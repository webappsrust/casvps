use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

mod config;
mod core;
mod database;
mod network;
mod services;
mod smart;
mod virtualization;
mod web;
mod clustering;
mod security;
mod iso;
mod backup;
mod monitoring;
mod certificates;
mod compliance;

use crate::core::CasVPS;

#[derive(Parser)]
#[command(
    name = "casvps",
    version = env!("CARGO_PKG_VERSION"),
    about = "Complete Application Server for Virtualization"
)]
struct Cli {
    #[arg(long, help = "Enable debug mode")]
    debug: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Start CasVPS service")]
    Start,

    #[command(about = "Stop CasVPS service")]
    Stop,

    #[command(about = "Restart CasVPS service")]
    Restart,

    #[command(about = "Show service status")]
    Status,

    #[command(about = "Node management commands")]
    Node {
        #[command(subcommand)]
        action: NodeCommands,
    },
}

#[derive(Subcommand)]
enum NodeCommands {
    #[command(about = "Add node to cluster")]
    Add {
        server: String,
        token: String,
    },

    #[command(about = "Remove node from cluster")]
    Remove {
        nodename: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let filter = if cli.debug {
        EnvFilter::new("debug")
    } else {
        EnvFilter::from_default_env()
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();

    // Handle commands
    match cli.command {
        Some(Commands::Start) | None => {
            info!("Starting CasVPS v{}", env!("CARGO_PKG_VERSION"));
            start_service().await?;
        }
        Some(Commands::Stop) => {
            info!("Stopping CasVPS");
            stop_service().await?;
        }
        Some(Commands::Restart) => {
            info!("Restarting CasVPS");
            stop_service().await?;
            start_service().await?;
        }
        Some(Commands::Status) => {
            show_status().await?;
        }
        Some(Commands::Node { action }) => {
            handle_node_command(action).await?;
        }
    }

    Ok(())
}

async fn start_service() -> Result<()> {
    // Initialize CasVPS
    let mut casvps = CasVPS::new().await?;

    // Start the service
    casvps.run().await?;

    Ok(())
}

async fn stop_service() -> Result<()> {
    // Send shutdown signal to running service
    if let Ok(mut client) = core::ServiceClient::connect().await {
        client.shutdown().await?;
    } else {
        error!("CasVPS service is not running");
    }

    Ok(())
}

async fn show_status() -> Result<()> {
    // Check if service is running
    match core::ServiceClient::connect().await {
        Ok(mut client) => {
            let status = client.get_status().await?;
            println!("CasVPS Status: {}", status);
        }
        Err(_) => {
            println!("CasVPS Status: Not running");
        }
    }

    Ok(())
}

async fn handle_node_command(cmd: NodeCommands) -> Result<()> {
    match cmd {
        NodeCommands::Add { server, token } => {
            info!("Adding node {} to cluster", server);
            let mut client = core::ServiceClient::connect().await?;
            client.join_cluster(&server, &token).await?;
            println!("Successfully joined cluster");
        }
        NodeCommands::Remove { nodename } => {
            info!("Removing node {} from cluster", nodename);
            let mut client = core::ServiceClient::connect().await?;
            client.leave_cluster(&nodename).await?;
            println!("Successfully removed node from cluster");
        }
    }

    Ok(())
}