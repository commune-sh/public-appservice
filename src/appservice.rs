use std::error;
use crate::config::Config;

use ruma::{
    api::client::{account::whoami, membership::joined_rooms, message::send_message_event},
    events::room::message::RoomMessageEventContent,
    OwnedRoomAliasId, TransactionId,
};
use anyhow;

pub type HttpClient = ruma::client::http_client::HyperNativeTls;

pub struct Client {
    client: ruma::Client<HttpClient>,
}

impl Client {
    pub async fn new(config: &Config) -> Result<Self, anyhow::Error> {

        let client = ruma::Client::builder()
            .homeserver_url(config.matrix.homeserver.clone())
            .access_token(Some(config.appservice.access_token.clone()))
            .build::<HttpClient>()
            .await
            .unwrap();

        Ok(Self { client })
    }

    pub async fn whoami(&self) -> Option<whoami::v3::Response> {
        self.client
            .send_request(whoami::v3::Request::new())
            .await
            .ok()
    }
}
