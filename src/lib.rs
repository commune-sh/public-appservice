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

