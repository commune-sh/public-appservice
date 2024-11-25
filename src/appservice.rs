use crate::config::Config;

use ruma::{
    api::client::{
        account::whoami, 
        membership::joined_rooms, 
        state::get_state_events,
        membership::{join_room_by_id, leave_room}
    },
    events::AnyStateEvent, 
    OwnedRoomId
};



use anyhow;

pub type HttpClient = ruma::client::http_client::HyperNativeTls;

#[derive(Clone)]
pub struct AppService {
    client: ruma::Client<HttpClient>,
}


type RoomState = Vec<ruma::serde::Raw<AnyStateEvent>>;

impl AppService {
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

    pub async fn join_room(&self, room_id: OwnedRoomId) {

        let jr = self.client
            .send_request(join_room_by_id::v3::Request::new(
                room_id
            ))
            .await
            .ok();
        println!("Join room: {:#?}", jr);
    }

    pub async fn leave_room(&self, room_id: OwnedRoomId) {

        let jr = self.client
            .send_request(leave_room::v3::Request::new(
                room_id
            ))
            .await
            .ok();
        println!("Left room: {:#?}", jr);
    }


    pub async fn joined_rooms(&self) -> Option<Vec<ruma::OwnedRoomId>> {

        let jr = self.client
            .send_request(joined_rooms::v3::Request::new())
            .await
            .ok()?;

        Some(jr.joined_rooms)

    }

    pub async fn joined_rooms_state(&self) -> Option<Vec<RoomState>> {

        let jr = self.client
            .send_request(joined_rooms::v3::Request::new())
            .await
            .ok()?;

        if jr.joined_rooms.len() == 0 {
            return None;
        }

        let mut rooms_state = Vec::new();

        for room_id in jr.joined_rooms {
            let st = self.client
                .send_request(get_state_events::v3::Request::new(
                    room_id,
                ))
                .await
                .ok()?;


            /*
            for state_event in &st.room_state {

                if state_event.get_field::<String>("type").ok()?.as_deref() == Some("m.room.create") {
                    let event = state_event.deserialize_as::<AnyStateEvent>().ok()?;

                    match event {
                        AnyStateEvent::RoomCreate(event) => {
                            println!("Event: {:#?}", event);
                        }
                        _ => {
                            println!("Unknown event: {:#?}", event);
                        }
                    }
                }

                /*

                if let Ok(Some(event)) = state_event.get_field::<String>("type") {
                    println!("Event type: {}", event);
                }
*/


            }
*/


            rooms_state.push(st.room_state);
        }

        Some(rooms_state)
    }
}
