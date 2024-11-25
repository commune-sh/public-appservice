use public_appservice::*; 

use config::Config;

use appservice::AppService;

use server::Server;

use ruma::{
    api::client::{account::whoami, membership::joined_rooms, message::send_message_event},
    events::room::message::RoomMessageEventContent,
    OwnedRoomAliasId, TransactionId,
};

type HttpClient = ruma::client::http_client::HyperNativeTls;

#[tokio::main]
async fn main() {

    // Read config
    let config = Config::new();


    let appservice = AppService::new(&config).await.unwrap();

    let whoami = appservice.whoami().await;

    match whoami {
        Some(whoami) => {
            println!("Logged in as: {:#?}", whoami);
        }
        None => {
            println!("Failed to get whoami");
        }
    }

    if let Some(rooms) = appservice.joined_rooms().await {
        println!("Joined rooms: {:#?}", rooms.len());
    }


    if let Some(room_states) = appservice.joined_rooms_state().await {
        println!("States: {:#?}", room_states.len());
    }

    let server = Server::new(config.clone(), appservice.clone());

    if let Err(e) = server.run(config.appservice.port.clone()).await {
        eprintln!("Server error: {}", e);
        std::process::exit(1);
    }

}
