use std::{path::PathBuf, process::ExitCode};

use clap::Parser;

use crate::{config::Config, logging, server::Server, Application};

/// Command line arguments
#[derive(Parser)]
#[clap(
    about,
    version = crate::version(),
)]
pub struct Args {
    #[clap(flatten)]
    pub(crate) config: ConfigArg,
}

#[derive(Parser)]
pub struct ConfigArg {
    #[arg(short, long)]
    pub path: Option<PathBuf>,
}

impl Args {
    pub async fn run() -> Result<(), ExitCode> {
        let args = Args::parse();

        let config = Config::new(args.config.path)?;

        logging::init(&config.tracing.filter)?;

        let state = Application::new(config).await.map_err(|error| {
            eprintln!("Failed to initialize state: {error}");

            ExitCode::FAILURE
        })?;

        tracing::info!("Starting Commune public appservice...");

        Server::new(state).run().await.map_err(|error| {
            eprintln!("Failed to start server: {error}");

            ExitCode::FAILURE
        })
    }
}
