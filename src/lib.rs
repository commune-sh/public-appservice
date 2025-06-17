pub mod config;
pub mod appservice;
pub mod server;
pub mod ping;
pub mod api;
pub mod rooms;
pub mod middleware;
pub mod cache;
pub mod error;
pub mod utils;
pub mod oidc;

use std::sync::Arc;
use axum::body::Body;
use hyper_util::{client::legacy::connect::HttpConnector, rt::TokioExecutor};

use hyper_tls::HttpsConnector;


pub type ProxyClient = hyper_util::client::legacy::Client<HttpsConnector<HttpConnector>, Body>;

#[derive(Clone)]
pub struct AppState {
    pub config: config::Config,
    pub proxy: ProxyClient,
    pub appservice: appservice::AppService,
    pub transaction_store: ping::TransactionStore,
    pub cache: cache::Cache,
    pub oidc: oidc::AuthMetadata,
}

impl AppState {
    pub async fn new(config: config::Config) -> Result<Arc<Self>, anyhow::Error> {

        let https = HttpsConnector::new();

        let client: ProxyClient =
            hyper_util::client::legacy::Client::<(), ()>::builder(TokioExecutor::new())
                .build(https);

        let appservice = appservice::AppService::new(&config).await?;

        let cache = cache::Cache::new(&config).await?;

        let transaction_store = ping::TransactionStore::new();

        let oidc = oidc::get_auth_metadata(&config.matrix.homeserver).await?;

        println!("OIDC Metadata: {:?}", oidc);

        Ok(Arc::new(Self {
            config,
            proxy: client,
            appservice,
            transaction_store,
            cache,
            oidc,
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

