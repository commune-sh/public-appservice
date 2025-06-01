pub mod api;
pub mod appservice;
pub mod args;
pub mod cache;
pub mod config;
pub mod constants;
pub mod error;
pub mod logging;
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

pub fn version() -> String {
    let version = env!("CARGO_PKG_VERSION");

    option_env!("GIT_COMMIT_HASH").map_or_else(
        || version.to_owned(),
        |commit_hash| format!("{version} ({commit_hash})"),
    )
}

#[derive(Clone)]
pub struct Application {
    pub config: config::Config,
    pub proxy: ProxyClient,
    pub appservice: appservice::AppService,
    pub txn_store: ping::TxnStore,
    pub cache: redis::Client,
    pub oidc: oidc::AuthMetadata,
}

impl Application {
    pub async fn new(config: config::Config) -> Result<Arc<Self>, anyhow::Error> {
        let client: ProxyClient =
            hyper_util::client::legacy::Client::<(), ()>::builder(TokioExecutor::new())
                .build(HttpConnector::new());

        let appservice = appservice::AppService::new(&config).await?;

        let cache = cache::Cache::new(&config).await?;

        let txn_store = ping::TxnStore::new();

        let oidc = oidc::get_auth_metadata(&config.matrix.homeserver).await?;

        println!("OIDC Metadata: {:?}", oidc);

        Ok(Arc::new(Self {
            config,
            proxy: client,
            appservice,
            txn_store,
            cache: cache.client,
            oidc,
        }))
    }
}
