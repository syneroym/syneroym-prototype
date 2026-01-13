use common::config::Config;
use anyhow::Result;
use std::future::Future;

pub trait NetworkInterface {
    fn init(config: &Config) -> impl Future<Output = Result<()>> + Send;
}