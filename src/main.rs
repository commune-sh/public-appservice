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

    /*
    let rooms = client
        .send_request(joined_rooms::v3::Request::new())
        .await;

    println!("Logged in as: {:#?}", client);
    println!("Logged in as: {:#?}", rooms);
*/

}
