mod args;
mod config;

use anyhow::Result;
use args::Cli;
use clap::Parser;
use config::Config;
use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};

const APP_ENV_VAR_PREFIX: &str = "SYNEROYM_";

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut fig = Figment::new().merge(Serialized::defaults(Config::default()));
    if let Some(config_file) = cli.config_file.as_deref() {
        fig = fig.merge(Toml::file(config_file));
    }

    let _conf: Config = fig.merge(Env::prefixed(APP_ENV_VAR_PREFIX)).extract()?;

    Ok(())
}
