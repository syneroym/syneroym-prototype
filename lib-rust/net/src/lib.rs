use anyhow::Result;
use common::config::Config;
use std::future::Future;

pub trait NetworkInterface {
    fn init(config: &Config) -> impl Future<Output = Result<()>> + Send;
}
