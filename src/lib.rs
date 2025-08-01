pub mod api;
pub mod appservice;
pub mod cache;
pub mod config;
pub mod error;
pub mod log;
pub mod middleware;
pub mod ping;
pub mod requests;
pub mod rooms;
pub mod server;
pub mod space;
pub mod utils;

use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;

pub type ProxyClient = reqwest::Client;

#[derive(Clone)]
pub struct AppState {
    pub config: config::Config,
    pub proxy: ProxyClient,
    pub appservice: appservice::AppService,
    pub transaction_store: ping::TransactionStore,
    pub cache: cache::Cache,
}

impl AppState {
    pub async fn new(config: config::Config) -> Result<Arc<Self>, anyhow::Error> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(90))
            .user_agent("commune-public-appservice")
            .build()?;

        let appservice = appservice::AppService::new(&config).await?;

        let cache = cache::Cache::new(&config).await?;

        let transaction_store = ping::TransactionStore::new();

        Ok(Arc::new(Self {
            config,
            proxy: client,
            appservice,
            transaction_store,
            cache,
        }))
    }
}

use clap::Parser;

#[derive(Parser)]
pub struct Args {
    #[arg(short, long, default_value = "config.toml")]
    pub config: std::path::PathBuf,
    #[arg(short, long, default_value = "8989")]
    pub port: u16,
}

impl Args {
    pub fn build() -> Self {
        Args::parse()
    }
}
