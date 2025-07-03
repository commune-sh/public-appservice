// src/logging.rs
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_level")]
    pub level: String,

    pub directory: Option<PathBuf>,

    pub filename: Option<String>,

    #[serde(default = "default_rotation")]
    pub rotation: String,
}

fn default_level() -> String {
    "info".to_string()
}
fn default_rotation() -> String {
    "daily".to_string()
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            directory: Some(PathBuf::from("./logs")),
            filename: Some("public-appservice.log".to_string()),
            rotation: "daily".to_string(),
        }
    }
}

pub fn setup_tracing(config: &LoggingConfig) -> anyhow::Result<Option<WorkerGuard>> {
    let env_filter = if cfg!(debug_assertions) {
        format!(
            "{},hyper_util=off,tower_http=off,ruma=off,reqwest=off",
            config.level
        )
    } else {
        config.level.clone()
    };

    let console_layer = tracing_subscriber::fmt::layer().pretty();

    if let (Some(log_directory), Some(log_filename)) = (&config.directory, &config.filename) {
        // File logging enabled
        if !log_directory.exists() {
            std::fs::create_dir_all(&log_directory)?;
        }

        let file_appender = match config.rotation.as_str() {
            "daily" => tracing_appender::rolling::daily(log_directory, log_filename),
            "hourly" => tracing_appender::rolling::hourly(log_directory, log_filename),
            "never" => tracing_appender::rolling::never(log_directory, log_filename),
            _ => tracing_appender::rolling::daily(log_directory, log_filename),
        };

        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(non_blocking)
            .with_ansi(false);

        tracing_subscriber::registry()
            .with(tracing_subscriber::EnvFilter::new(env_filter))
            .with(console_layer)
            .with(file_layer)
            .init();

        tracing::info!("Tracing initialized with file logging");
        Ok(Some(guard))
    } else {
        // Console only
        tracing_subscriber::registry()
            .with(tracing_subscriber::EnvFilter::new(env_filter))
            .with(console_layer)
            .init();

        tracing::info!("Tracing initialized with console logging only");
        Ok(None)
    }
}
