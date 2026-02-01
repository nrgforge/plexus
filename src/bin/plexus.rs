//! Plexus CLI — knowledge graph engine with MCP server.
//!
//! Usage:
//!   plexus mcp [--transport stdio] [--db path]

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "plexus",
    version,
    about = "Network-aware knowledge graph engine"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the MCP (Model Context Protocol) server
    Mcp {
        /// Transport type (currently only stdio)
        #[arg(long, default_value = "stdio")]
        transport: String,
        /// Path to SQLite database file
        #[arg(long)]
        db: Option<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Mcp { transport, db } => {
            if transport != "stdio" {
                eprintln!("error: only 'stdio' transport is currently supported");
                std::process::exit(1);
            }
            let code = plexus::mcp::run_mcp_server(db);
            std::process::exit(code);
        }
    }
}
