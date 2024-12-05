use public_appservice::*; 
use config::Config;
use appservice::AppService;
use server::Server;
use cache::Cache;

use std::sync::Arc;

use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{AppState, ProxyClient};
use hyper_util::{client::legacy::connect::HttpConnector, rt::TokioExecutor};

use crate::ping::TransactionStore;


#[tokio::main]
async fn main() {

    setup_tracing();

    let config = Config::new();

    let appservice = AppService::new(&config)
        .await
        .unwrap_or_else(|e| {
            eprintln!("Can't reach homeserver: {}", e);
            std::process::exit(1);
        });

    let cache = Cache::new(&config)
        .await
        .unwrap_or_else(|e| {
            eprintln!("Can't connect to Redis: {}", e);
            std::process::exit(1);
        });

    info!("Starting Commune public appservice...");

    Server::new(
        config.clone(), 
        appservice.clone(), 
        cache
    )
    .run()
    .await 
    .unwrap_or_else(|e| {
        eprintln!("Server error: {}", e);
        std::process::exit(1);
    }); 

}

pub fn new_state(
    config: Config, 
    appservice: AppService,
    cache: Cache,
) -> Result<Arc<AppState>, anyhow::Error> {

    let client: ProxyClient =
    hyper_util::client::legacy::Client::<(), ()>::builder(TokioExecutor::new())
        .build(HttpConnector::new());


    let transaction_store = TransactionStore::new();

    let state = Arc::new(AppState {
        config: config.clone(),
        proxy: client,
        appservice: appservice.clone(),
        transaction_store,
        cache: cache.client.clone(),
    });

    Ok(state)
}

pub fn setup_tracing() {
    let env_filter = if cfg!(debug_assertions) {
        "debug,hyper_util=off,tower_http=off,ruma=off,reqwest=off"
    } else {
        "info"
    };

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::new(env_filter))
        .init();
}
