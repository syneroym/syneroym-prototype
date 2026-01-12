use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: CliCommand,

    // Global options
    /// Config file path
    #[arg(short, long, global = true, value_name = "FILE")]
    pub config_file: Option<PathBuf>,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(Debug, Subcommand)]
pub enum CliCommand {
    /// Run peer
    RunPeer(RunPeerArgs),
    /// Show version information
    Version,
}

#[derive(Debug, Parser)]
pub struct RunPeerArgs {
    /// Enable http transport support
    #[arg(short, long)]
    enable_http_txp: bool,
}
