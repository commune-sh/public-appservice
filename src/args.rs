use std::path::PathBuf;

use clap::Parser;

use crate::{config::Config, error::startup::Main as Error, logging, server::Server, Application};

#[derive(Parser)]
#[clap(
    about,
    version = crate::version(),
)]
pub struct Args {
    #[clap(flatten)]
    pub config: ConfigArg,
}

#[derive(Parser)]
pub struct ConfigArg {
    #[arg(short, long)]
    pub path: Option<PathBuf>,
}

impl Args {
    pub async fn run() -> Result<(), Error> {
        let args = Args::try_parse()?;

        let config = Config::new(args.config.path).map_err(Error::Config)?;

        logging::init(&config.tracing.filter)?;

        let state = Application::new(config).await?;

        tracing::info!("Starting Commune public appservice...");

        Server::new(state).run().await?;

        Ok(())
    }
}
