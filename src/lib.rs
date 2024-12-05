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

use axum::body::Body;
use hyper_util::{client::legacy::connect::HttpConnector, rt::TokioExecutor};

pub type ProxyClient = hyper_util::client::legacy::Client<HttpConnector, Body>;

#[derive(Clone)]
pub struct AppState {
    pub config: config::Config,
    pub proxy: ProxyClient,
    pub appservice: appservice::AppService,
    pub transaction_store: ping::TransactionStore,
    pub cache: redis::Client,
}

impl AppState {
    pub async fn new(config: config::Config) -> Result<Self, anyhow::Error> {
        let client: ProxyClient =
            hyper_util::client::legacy::Client::<(), ()>::builder(TokioExecutor::new())
                .build(HttpConnector::new());

        let appservice = appservice::AppService::new(&config).await?;
        let cache = cache::Cache::new(&config).await?;

        let transaction_store = ping::TransactionStore::new();

        Ok(Self {
            config,
            proxy: client,
            appservice,
            transaction_store,
            cache: cache.client,
        })
    }
}
