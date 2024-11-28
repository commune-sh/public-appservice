use public_appservice::*; 
use config::Config;
use appservice::AppService;
use server::Server;

use tracing::info;
use tracing_subscriber;

#[tokio::main]
async fn main() {

    tracing_subscriber::fmt::init();

    // Read config
    let config = Config::new();

    let appservice = AppService::new(&config).await.unwrap();

    let server = Server::new(config.clone(), appservice.clone());

    info!("Starting Commune public appservice...");

    if let Err(e) = server.run(config.appservice.port.clone()).await {
        eprintln!("Server error: {}", e);
        std::process::exit(1);
    }

}
