use public_appservice::*; 
use config::Config;
use appservice::AppService;
use server::Server;

use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {

    setup_tracing();

    let config = Config::new();

    let appservice = AppService::new(&config)
        .await
        .unwrap_or_else(|e| {
            eprintln!("Can't reach homeserver: {}", e);
            std::process::exit(1);
        });

    let server = Server::new(config.clone(), appservice.clone());

    info!("Starting Commune public appservice...");

    if let Err(e) = server.run(config.server.port.clone()).await {
        eprintln!("Server error: {}", e);
        std::process::exit(1);
    }

}

pub fn setup_tracing() {
    let env_filter = if cfg!(debug_assertions) {
        "debug,hyper_util=off,tower_http=off,ruma=off,reqwest=off"
    } else {
        "info"
    };

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::new(env_filter))
        .init();
}
