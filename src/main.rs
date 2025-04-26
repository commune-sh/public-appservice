use std::process::ExitCode;

use config::Config;
use public_appservice::*;
use server::Server;

use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::AppState;

#[tokio::main]
async fn main() -> ExitCode {
    setup_tracing();

    let args = Args::build();

    let config = Config::new(&args.config);

    let Ok(state) = AppState::new(config.clone()).await else {
        eprintln!("Failed to initialize state: {error}");

        return ExitCode::FAILURE;
    };

    info!("Starting Commune public appservice...");

    if let Err(error) = Server::new(state).run().await {
        eprintln!("Error: {error}");

        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
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
