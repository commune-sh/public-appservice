use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Main {
    #[error(transparent)]
    Args(#[from] clap::Error),

    #[error(transparent)]
    Config(Config),

    #[error("failed to initialize tracing subscriber")]
    Tracing(#[from] tracing::subscriber::SetGlobalDefaultError),

    // TODO: cannot have two impls
    // #[error("failed to initialize application")]
    // Initialize(#[from] anyhow::Error),
    #[error("failed to serve requests")]
    Serve(#[from] anyhow::Error),
}

#[derive(Debug, Error)]
pub enum Config {
    #[error("no config found: {0:?}")]
    Search(PathBuf),

    #[error("failed to read config: {1:?}")]
    Read(#[source] std::io::Error, PathBuf),

    #[error("failed to parse config: {1:?}")]
    Parse(#[source] toml::de::Error, PathBuf),
}
