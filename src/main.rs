use public_appservice::*; 
use config::Config;
use server::Server;

use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::AppState;

#[tokio::main]
async fn main() {

    setup_tracing();

    let args = Args::build();

    let config = Config::new(&args.config);

    let state = AppState::new(config.clone())
        .await
        .unwrap_or_else(|e| {
            eprintln!("Failed to initialize state: {}", e);
            std::process::exit(1);
        });

    info!("Starting Commune public appservice...");

    Server::new(state)
    .run()
    .await 
    .unwrap_or_else(|e| {
        eprintln!("Server error: {}", e);
        std::process::exit(1);
    }); 

}

pub fn setup_tracing() {
    let env_filter = if cfg!(debug_assertions) {
        "debug,hyper_util=off,tower_http=off,ruma=off,reqwest=off,aws_runtime=off,aws_sdk_s3=off,aws_smithy_runtime=off,aws_smithy_runtime_api=off"

    } else {
        "info"
    };

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::new(env_filter))
        .init();
}
