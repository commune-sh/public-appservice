use sqlx::postgres::{PgPool, PgPoolOptions, PgConnectOptions};
use sqlx::ConnectOptions;
use std::process;

use crate::config::Config;

#[derive(Clone)]
pub struct Database {
    pub pool: PgPool,
}

impl Database {
    pub async fn new(config: &Config) -> Self {

        let pool: PgPool;
        let mut opts: PgConnectOptions = config.db.url.clone().parse().unwrap();
        opts = opts.log_statements(tracing::log::LevelFilter::Debug)
               .log_slow_statements(tracing::log::LevelFilter::Warn, std::time::Duration::from_secs(1));


        let pg_pool = PgPoolOptions::new()
            .max_connections(5)
            .min_connections(1)
            .connect_with(opts)
            .await;

        match pg_pool {
            Ok(p) => {
                tracing::info!("Successfully connected to database");
                pool = p
            }
            Err(e) => {
                tracing::error!("Database Error:");

                let mut error: &dyn std::error::Error = &e;
                tracing::error!("Error: {}", error);

                while let Some(source) = error.source() {
                    tracing::error!("Caused by: {}", source);
                    error = source;
                }
                tracing::error!("Public appservice cannot start without a valid database connection");

                process::exit(1);
            }
        }

        Self {
            pool: pool.clone(),
        }

    }
}

