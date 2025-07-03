use config::Config;
use public_appservice::*;
use server::Server;

use tracing::info;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::AppState;

#[tokio::main]
async fn main() {
    let args = Args::build();

    let config = Config::new(&args.config);

    let _logging_guard = setup_tracing(&config);

    let state = AppState::new(config.clone()).await.unwrap_or_else(|e| {
        tracing::info!("Failed to initialize state: {}", e);
        std::process::exit(1);
    });

    info!("Starting Commune public appservice...");

    Server::new(state).run().await.unwrap_or_else(|e| {
        tracing::info!("Server error: {}", e);
        std::process::exit(1);
    });
}

pub fn setup_tracing(config: &Config) -> WorkerGuard {
    let env_filter = if cfg!(debug_assertions) {
        "debug,hyper_util=off,tower_http=off,ruma=off,reqwest=off"
    } else {
        "info"
    };

    let log_directory = match &config.logging {
        Some(logging) => logging.directory.clone(),
        None => "./logs".to_string(),
    };

    if !std::path::Path::new(&log_directory).exists() {
        std::fs::create_dir_all(&log_directory).unwrap_or_else(|e| {
            tracing::info!("Failed to create log directory: {}", e);
            std::process::exit(1);
        });
    }

    let log_filename = match &config.logging {
        Some(logging) => logging.filename.clone(),
        None => "commune.log".to_string(),
    };

    let file_appender = tracing_appender::rolling::daily(log_directory, log_filename);

    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let console_layer = tracing_subscriber::fmt::layer().pretty();

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false);

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(env_filter))
        .with(console_layer)
        .with(file_layer)
        .init();

    tracing::info!("Tracing initialized with file logging");

    guard
}
