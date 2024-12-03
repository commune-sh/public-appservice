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

#[derive(Clone)]
pub struct AppState {
    pub config: config::Config,
    pub client: server::Client,
    pub appservice: appservice::AppService,
    pub transaction_store: ping::TransactionStore,
    pub cache: redis::Client,
}

