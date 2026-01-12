use clap::{Parser, Subcommand};
use figment::Figment;
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

impl Cli {
    pub fn update_figment(&self, mut fig: Figment) -> Figment {
        match &self.command {
            CliCommand::RunPeer(args) => {
                if args.enable_http_txp {
                    fig = fig.merge(("http_txp", true));
                }
                if let Some(ref secret_key_path) = args.secret_key_path {
                    fig = fig.merge(("iroh_comm.secret_key_path", secret_key_path));
                }
            }
            CliCommand::Version => {}
        }
        fig
    }
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
    pub enable_http_txp: bool,

    /// Secret key file path (overrides config)
    #[arg(long, value_name = "FILE")]
    pub secret_key_path: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::config::Config;
    use figment::providers::Serialized;

    #[test]
    fn test_update_figment_defaults() {
        let cli = Cli {
            command: CliCommand::RunPeer(RunPeerArgs {
                enable_http_txp: false,
                secret_key_path: None,
            }),
            config_file: None,
            verbose: false,
        };

        let fig = Figment::new().merge(Serialized::defaults(Config::default()));
        let fig = cli.update_figment(fig);
        let conf: Config = fig.extract().unwrap();

        // Default should remain
        assert!(!conf.http_txp); // Default is false now
        if let Some(iroh) = conf.iroh_comm {
             assert_eq!(iroh.secret_key_path, None);
        }
    }

    #[test]
    fn test_update_figment_overrides() {
        let path = PathBuf::from("/tmp/secret");
        let cli = Cli {
            command: CliCommand::RunPeer(RunPeerArgs {
                enable_http_txp: true, // It's already true by default, but let's pass true
                secret_key_path: Some(path.clone()),
            }),
            config_file: None,
            verbose: false,
        };

        let mut conf = Config::default();
        conf.http_txp = false; // Set default to false to test override if logic allows, 
                               // but update_figment only merges if arg is true?
                               // Wait, args.enable_http_txp is a flag. If present (true), it sets to true.
                               // If not present (false), it does nothing (leaves default).
        
        let fig = Figment::new().merge(Serialized::defaults(conf)); // Start with false
        let fig = cli.update_figment(fig);
        let conf: Config = fig.extract().unwrap();

        assert!(conf.http_txp);
        assert_eq!(conf.iroh_comm.unwrap().secret_key_path, Some(path));
    }
}