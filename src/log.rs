use crate::config::Config;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use sentry::ClientInitGuard;
use sentry_tracing::EventFilter;
use metrics_exporter_prometheus::PrometheusBuilder;

pub fn setup_sentry(config: &Config) -> Option<ClientInitGuard> {
    match config.sentry {
        Some(ref sentry_config) => {
            if sentry_config.enabled && sentry_config.dsn != "" {
                let dsn = sentry_config.dsn.clone();
                tracing::info!("Sentry is enabled with DSN.");
                let guard = sentry::init((
                    dsn,
                    sentry::ClientOptions {
                        release: sentry::release_name!(),
                        traces_sample_rate: 1.0,
                        debug: true,
                        ..Default::default()
                    },
                ));
                Some(guard)
            } else {
                None
            }
        },
        None => {
            None
        }
    }
}

pub fn setup_tracing(config: &Config) -> Result<WorkerGuard, anyhow::Error> {
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
        std::fs::create_dir_all(&log_directory)?;
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
        .with(sentry_tracing::layer().event_filter(|md| match md.level() {
            &tracing::Level::ERROR => EventFilter::Breadcrumb,
            &tracing::Level::INFO => EventFilter::Event,
            &tracing::Level::WARN => EventFilter::Event,
            _ => EventFilter::Ignore,
        }))
        .with(tracing_subscriber::EnvFilter::new(env_filter))
        .with(console_layer)
        .with(file_layer)
        .init();

    tracing::info!("Tracing initialized with file logging");

    Ok(guard)
}


pub fn setup_metrics(config: &Config) -> anyhow::Result<()> {
    let builder = PrometheusBuilder::new();

    if !config.metrics.enabled || config.metrics.port == 0 {
        tracing::info!("Metrics is disabled.");
        return Ok(());
    }

    builder
        .with_http_listener(([0, 0, 0, 0], config.metrics.port))
        .install()?;
    tracing::info!("Metrics endpoint at http://localhost:{}/metrics", config.metrics.port);
    
    Ok(())
}
