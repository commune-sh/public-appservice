use std::path::PathBuf;

use clap::Parser;

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
    pub config: Option<PathBuf>,
}
