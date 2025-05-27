pub mod api;
pub mod appservice;
pub mod cache;
pub mod config;
pub mod error;
pub mod middleware;
pub mod oidc;
pub mod ping;
pub mod rooms;
pub mod server;
pub mod utils;

use axum::body::Body;
use hyper_util::{client::legacy::connect::HttpConnector, rt::TokioExecutor};
use std::sync::Arc;

pub type ProxyClient = hyper_util::client::legacy::Client<HttpConnector, Body>;

#[derive(Clone)]
pub struct AppState {
    pub config: config::Config,
    pub proxy: ProxyClient,
    pub appservice: appservice::AppService,
    pub transaction_store: ping::TransactionStore,
    pub cache: redis::Client,
    pub oidc: oidc::AuthMetadata,
}

impl AppState {
    pub async fn new(config: config::Config) -> Result<Arc<Self>, anyhow::Error> {
        let client: ProxyClient =
            hyper_util::client::legacy::Client::<(), ()>::builder(TokioExecutor::new())
                .build(HttpConnector::new());

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
            cache: cache.client,
            oidc,
        }))
    }
}
