use crate::config::Config;

use ruma::{
    OwnedRoomId,
    OwnedEventId,
    api::client::{
        alias::get_alias,
        account::whoami, 
        membership::joined_rooms, 
        state::{
            get_state_events, 
            get_state_events_for_key
        },
        room::get_room_event,
        membership::{
            join_room_by_id, 
            leave_room
        }
    },
    events::{
        AnyTimelineEvent,
        AnyStateEvent, 
        StateEventType,
        room::{
            name::RoomNameEventContent,
            canonical_alias::RoomCanonicalAliasEventContent,
            avatar::RoomAvatarEventContent,
            topic::RoomTopicEventContent,
        }
    }
};

use anyhow;

use serde::{Serialize, Deserialize};

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

        }

        Some(joined_rooms)
    }

    pub async fn get_room_event(&self, room_id: OwnedRoomId, event_id: OwnedEventId) -> Option<ruma::serde::Raw<AnyTimelineEvent>> {

        let event = self.client
            .send_request(get_room_event::v3::Request::new(
                room_id,
                event_id,
            ))
            .await
            .ok()?;

        Some(event.event)
    }

    pub async fn get_room_summary(&self, room_id: OwnedRoomId) ->
    Option<RoomSummary> {

        let mut room_info = RoomSummary {
            room_id: room_id.to_string(),
            ..Default::default()
        };

        let state = self.client
            .send_request(get_state_events::v3::Request::new(
                room_id,
            ))
            .await
            .ok()?;

        for state_event in state.room_state {

            let event_type = match state_event.get_field::<String>("type") {
                Ok(Some(t)) => t,
                Ok(None) => {
                    continue;
                }
                Err(_) => {
                    continue;
                }
            };

            if event_type == "m.room.name" {
                if let Ok(Some(content)) = state_event.get_field::<RoomNameEventContent>("content") {
                    room_info.name = Some(content.name.to_string());
                };
            }

            if event_type == "m.room.canonical_alias" {
                if let Ok(Some(content)) = state_event.get_field::<RoomCanonicalAliasEventContent>("content") {
                    room_info.canonical_alias = content.alias.map(|a| a.to_string());
                };
            }

            if event_type == "m.room.avatar" {
                if let Ok(Some(content)) = state_event.get_field::<RoomAvatarEventContent>("content") {
                    room_info.avatar_url = content.url.map(|u| u.to_string());
                };
            }

            if event_type == "commune.room.banner" {
                if let Ok(Some(content)) = state_event.get_field::<RoomAvatarEventContent>("content") {
                    room_info.avatar_url = content.url.map(|u| u.to_string());
                };
            }

            if event_type == "m.room.topic" {
                if let Ok(Some(content)) = state_event.get_field::<RoomTopicEventContent>("content") {
                    room_info.topic = Some(content.topic.to_string());
                };
            }
        }

        Some(room_info)
    }

}

#[derive(Default, Debug, Deserialize, Serialize)]
pub struct RoomSummary {
    pub room_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub banner_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
}
