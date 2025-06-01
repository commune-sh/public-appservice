use std::process::ExitCode;

use commune::args;

#[tokio::main]
pub async fn main() -> ExitCode {
    let Err(_error) = args::Args::run().await else {
        return ExitCode::SUCCESS;
    };

    ExitCode::FAILURE
}
