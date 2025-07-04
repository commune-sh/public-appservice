use config::Config;
use public_appservice::*;
use server::Server;

use tracing::info;

use crate::AppState;

use anyhow::Context;


use log::{
    setup_tracing,
    setup_metrics,
    setup_sentry,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::build();
    let config = Config::new(&args.config)?;
    
    let _sentry_guard = setup_sentry(&config);
    let _logging_guard = setup_tracing(&config);
    setup_metrics(&config)?;

    let state = AppState::new(config.clone())
        .await
        .context("Failed to initialize application state")?;

    info!("Starting Commune public appservice...");

    Server::new(state)
        .run()
        .await
        .context("Server encountered an error")?;

    Ok(())
}
