use crate::config::Config;

use ruma::{
    api::client::{
        alias::get_alias,
        account::whoami, 
        membership::joined_rooms, 
        state::{get_state_events, get_state_events_for_key},
        membership::{join_room_by_id, leave_room}
    },
    events::{AnyStateEvent, StateEventType}, 
    OwnedRoomId
};



use anyhow;

pub type HttpClient = ruma::client::http_client::HyperNativeTls;

#[derive(Clone)]
pub struct AppService {
    client: ruma::Client<HttpClient>,
    pub user_id: String,
}


pub type RoomState = Vec<ruma::serde::Raw<AnyStateEvent>>;

#[derive(Clone)]
pub struct JoinedRoomState {
    pub room_id: OwnedRoomId,
    pub state: Option<RoomState>,
}

impl AppService {
    pub async fn new(config: &Config) -> Result<Self, anyhow::Error> {

        let client = ruma::Client::builder()
            .homeserver_url(config.matrix.homeserver.clone())
            .access_token(Some(config.appservice.access_token.clone()))
            .build::<HttpClient>()
            .await
            .unwrap();

        let user_id = format!("@{}:{}", config.appservice.sender_localpart, config.matrix.server_name);

        Ok(Self { client, user_id })
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

    pub async fn has_joined_room(&self, room_id: OwnedRoomId) -> bool {

        let jr = self.client
            .send_request(get_state_events_for_key::v3::Request::new(
                room_id,
                StateEventType::RoomMember,
                self.user_id.clone(),
            ))
            .await 
            .ok();

        println!("Has joined room: {:#?}", jr);

        jr.is_some()
    }

    pub async fn get_room_state(&self, room_id: OwnedRoomId) ->
    Option<RoomState> {

        let state = self.client
            .send_request(get_state_events::v3::Request::new(
                room_id,
            ))
            .await
            .ok()?;

        Some(state.room_state)
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

    pub async fn room_id_from_alias(&self, room_alias: ruma::OwnedRoomAliasId) -> Option<ruma::OwnedRoomId> {

        let room_id = self.client
            .send_request(get_alias::v3::Request::new(
                room_alias,
            ))
            .await
            .ok()?;

        Some(room_id.room_id)
    }


    pub async fn joined_rooms_state(&self) -> Option<Vec<JoinedRoomState>> {

        let mut joined_rooms: Vec<JoinedRoomState> = Vec::new();

        let jr = self.client
            .send_request(joined_rooms::v3::Request::new())
            .await
            .ok()?;

        if jr.joined_rooms.len() == 0 {
            return None;
        }

        for room_id in jr.joined_rooms {

            let mut jrs = JoinedRoomState {
                room_id: room_id.clone(),
                state: None,
            };


            let st = self.client
                .send_request(get_state_events::v3::Request::new(
                    room_id,
                ))
                .await
                .ok()?;

            jrs.state = Some(st.room_state);

            joined_rooms.push(jrs);


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


            //rooms_state.push(st.room_state);
        }

        Some(joined_rooms)
    }
}
