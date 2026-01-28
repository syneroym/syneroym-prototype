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
                if let Some(ref secret_key_path) = args.secret_key_path {
                    fig = fig.merge(("comm_iroh.secret_key_path", secret_key_path));
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
                secret_key_path: None,
            }),
            config_file: None,
            verbose: false,
        };

        let fig = Figment::new().merge(Serialized::defaults(Config::default()));
        let fig = cli.update_figment(fig);
        let conf: Config = fig.extract().unwrap();

        assert_eq!(conf.enabled_comms, vec!["iroh".to_string()]);
        if let Some(iroh) = conf.comm_iroh {
            assert_eq!(iroh.secret_key_path, None);
        }
    }

    #[test]
    fn test_update_figment_overrides() {
        let path = PathBuf::from("/tmp/secret");
        let cli = Cli {
            command: CliCommand::RunPeer(RunPeerArgs {
                secret_key_path: Some(path.clone()),
            }),
            config_file: None,
            verbose: false,
        };

        let conf = Config::default();

        let fig = Figment::new().merge(Serialized::defaults(conf)); // Start with false
        let fig = cli.update_figment(fig);
        let conf: Config = fig.extract().unwrap();

        assert_eq!(conf.comm_iroh.unwrap().secret_key_path, Some(path));
    }
}
