use std::process::ExitCode;

use clap::Parser as _;
use cli::Args;
use config::Config;
use public_appservice::*;
use server::Server;

use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::AppState;

mod cli;

#[tokio::main]
async fn main() -> Result<(), ExitCode> {
    setup_tracing();

    let args = Args::parse();

    let config = Config::new(&args.config);

    let state = AppState::new(config.clone()).await.map_err(|error| {
        eprintln!("Failed to initialize state: {error}");

        ExitCode::FAILURE
    })?;

    info!("Starting Commune public appservice...");

    Server::new(state).run().await.map_err(|error| {
        eprintln!("Failed to start server: {error}");

        ExitCode::FAILURE
    })
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
