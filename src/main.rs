use config::Config;
use public_appservice::*;
use server::Server;

use tracing::info;

use log::setup_tracing;

use crate::AppState;

#[tokio::main]
async fn main() {
    let args = Args::build();

    let config = Config::new(&args.config);

    let _logging_guard = setup_tracing(&config.logging).unwrap_or_else(|e| {
        tracing::warn!("Failed to initialize tracing: {}", e);
        std::process::exit(1);
    });

    let state = AppState::new(config.clone()).await.unwrap_or_else(|e| {
        tracing::warn!("Failed to initialize state: {}", e);
        std::process::exit(1);
    });

    info!("Starting Commune public appservice...");

    Server::new(state).run().await.unwrap_or_else(|e| {
        tracing::warn!("Server error: {}", e);
        std::process::exit(1);
    });
}
