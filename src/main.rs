mod config;
use config::Config;

mod appservice;
use appservice::Client;

use ruma::{
    api::client::{account::whoami, membership::joined_rooms, message::send_message_event},
    events::room::message::RoomMessageEventContent,
    OwnedRoomAliasId, TransactionId,
};

type HttpClient = ruma::client::http_client::HyperNativeTls;

struct AppService {
    config: Config,
    client: ruma::Client<HttpClient>,
}

#[tokio::main]
async fn main() {
    // Read config
    let config = Config::new();

    config.print();

    let client = Client::new(&config).await.unwrap();

    let whoami = client.whoami().await;

    match whoami {
        Some(whoami) => {
            println!("Logged in as: {:#?}", whoami);
        }
        None => {
            println!("Failed to get whoami");
        }
    }

    if let Some(rooms) = client.joined_rooms().await {
        println!("Joined rooms: {:#?}", rooms);
        println!("Joined rooms: {:#?}", rooms.len());
    }


    if let Some(room_states) = client.joined_rooms_state().await {
        println!("States: {:#?}", room_states);
    }

}
