use crate::config::Config;

use ruma::{
    api::client::{
        account::whoami,
        alias::get_alias,
        appservice::request_ping,
        membership::{join_room_by_id, joined_rooms, leave_room},
        profile::get_profile,
        room::get_room_event,
        space::{get_hierarchy, SpaceHierarchyRoomsChunk},
        state::{get_state_events, get_state_events_for_key},
    },
    events::{
        room::{
            avatar::RoomAvatarEventContent, canonical_alias::RoomCanonicalAliasEventContent,
            name::RoomNameEventContent, topic::RoomTopicEventContent,
        },
        AnyStateEvent, AnyTimelineEvent, StateEventType,
    },
    OwnedEventId, OwnedRoomId, OwnedTransactionId, OwnedUserId, RoomAliasId, UserId,
};

use anyhow;

use serde::{Deserialize, Serialize};

pub type HttpClient = ruma::client::http_client::HyperNativeTls;

#[derive(Clone)]
pub struct AppService {
    client: ruma::Client<HttpClient>,
    config: Config,
    pub appservice_id: String,
    pub user_id: Box<OwnedUserId>,
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
            .await?;

        let user_id = UserId::parse(format!(
            "@{}:{}",
            config.appservice.sender_localpart, config.matrix.server_name
        ))?;

        let whoami = client.send_request(whoami::v3::Request::new()).await;

        if whoami.is_err() {
            eprintln!("Failed to authenticate with homeserver. Check your access token.");
            std::process::exit(1);
        }

        Ok(Self {
            client,
            config: config.clone(),
            appservice_id: config.appservice.id.clone(),
            user_id: Box::new(user_id),
        })
    }

    pub async fn ping_homeserver(
        &self,
        id: String,
    ) -> Result<request_ping::v1::Response, anyhow::Error> {
        let mut req = request_ping::v1::Request::new(self.appservice_id.to_string());

        req.transaction_id = Some(OwnedTransactionId::from(id));

        let response = self.client.send_request(req).await?;
        Ok(response)
    }

    pub fn user_id(&self) -> String {
        self.user_id.to_string()
    }

    pub async fn whoami(&self) -> Result<whoami::v3::Response, anyhow::Error> {
        let r = self.client.send_request(whoami::v3::Request::new()).await?;
        Ok(r)
    }

    pub async fn join_room(&self, room_id: OwnedRoomId) -> Result<(), anyhow::Error> {
        let jr = self
            .client
            .send_request(join_room_by_id::v3::Request::new(room_id))
            .await?;

        tracing::info!("Joined room: {:#?}", jr);
        Ok(())
    }

    pub async fn has_joined_room(&self, room_id: OwnedRoomId) -> Result<bool, anyhow::Error> {
        let jr = self
            .client
            .send_request(get_state_events_for_key::v3::Request::new(
                room_id,
                StateEventType::RoomMember,
                self.user_id(),
            ))
            .await?;

        let membership = jr.content.get_field::<String>("membership")?;

        Ok(membership == Some("join".to_string()))
    }

    pub async fn get_room_state(&self, room_id: OwnedRoomId) -> Result<RoomState, anyhow::Error> {
        let state = self
            .client
            .send_request(get_state_events::v3::Request::new(room_id))
            .await?;

        Ok(state.room_state)
    }

    pub async fn leave_room(&self, room_id: OwnedRoomId) -> Result<(), anyhow::Error> {
        // First leave all child rooms
        let hierarchy = self
            .client
            .send_request(get_hierarchy::v1::Request::new(room_id.clone()))
            .await?;

        println!("Hierarchy rooms: {:#?}", hierarchy.rooms.len());

        for room in hierarchy.rooms {
            if room.room_id == room_id {
                continue;
            }
            let left = self
                .client
                .send_request(leave_room::v3::Request::new(room.room_id.clone()))
                .await?;
            println!("Left child room: {:#?}", room.room_id);
            tracing::info!("Left child room: {:#?}", left);
        }

        let left = self
            .client
            .send_request(leave_room::v3::Request::new(room_id))
            .await?;

        tracing::info!("Left room: {:#?}", left);

        Ok(())
    }

    pub async fn joined_rooms(&self) -> Result<Vec<ruma::OwnedRoomId>, anyhow::Error> {
        let jr = self
            .client
            .send_request(joined_rooms::v3::Request::new())
            .await?;

        Ok(jr.joined_rooms)
    }

    pub async fn room_id_from_alias(
        &self,
        room_alias: ruma::OwnedRoomAliasId,
    ) -> Result<ruma::OwnedRoomId, anyhow::Error> {
        let room_id = self
            .client
            .send_request(get_alias::v3::Request::new(room_alias))
            .await?;

        Ok(room_id.room_id)
    }

    pub async fn joined_rooms_state(&self) -> Result<Option<Vec<JoinedRoomState>>, anyhow::Error> {
        let curated = self.config.public_rooms.curated;
        let include_rooms = &self.config.public_rooms.include_rooms;

        if curated && !include_rooms.is_empty() {
            // Get subset of joined rooms from config
            let mut joined_rooms: Vec<JoinedRoomState> = Vec::new();

            // first get top level spaces
            for local_part in include_rooms {
                let alias = format!("#{}:{}", local_part, self.config.matrix.server_name);

                let alias = RoomAliasId::parse(&alias)?;

                let room_id = self.room_id_from_alias(alias).await?;

                let mut jrs = JoinedRoomState {
                    room_id: room_id.clone(),
                    state: None,
                };

                let st = self
                    .client
                    .send_request(get_state_events::v3::Request::new(room_id.clone()))
                    .await?;

                jrs.state = Some(st.room_state);

                joined_rooms.push(jrs);

                // find child rooms and add to list
                let hierarchy = self
                    .client
                    .send_request(get_hierarchy::v1::Request::new(room_id))
                    .await?;

                for room in hierarchy.rooms {
                    let mut jrs = JoinedRoomState {
                        room_id: room.room_id.clone(),
                        state: None,
                    };
                    let st = self
                        .client
                        .send_request(get_state_events::v3::Request::new(room.room_id.clone()))
                        .await?;

                    jrs.state = Some(st.room_state);

                    let exists = joined_rooms.iter().any(|r| r.room_id == room.room_id);
                    if !exists {
                        joined_rooms.push(jrs);
                    }
                }
            }

            return Ok(Some(joined_rooms));
        }

        let mut joined_rooms: Vec<JoinedRoomState> = Vec::new();

        let jr = self
            .client
            .send_request(joined_rooms::v3::Request::new())
            .await?;

        if jr.joined_rooms.is_empty() {
            return Ok(None);
        }

        for room_id in jr.joined_rooms {
            let mut jrs = JoinedRoomState {
                room_id: room_id.clone(),
                state: None,
            };

            let st = self
                .client
                .send_request(get_state_events::v3::Request::new(room_id))
                .await?;

            jrs.state = Some(st.room_state);

            joined_rooms.push(jrs);
        }

        Ok(Some(joined_rooms))
    }

    pub async fn get_room_event(
        &self,
        room_id: OwnedRoomId,
        event_id: OwnedEventId,
    ) -> Result<ruma::serde::Raw<AnyTimelineEvent>, anyhow::Error> {
        let event = self
            .client
            .send_request(get_room_event::v3::Request::new(room_id, event_id))
            .await?;

        Ok(event.event)
    }

    pub async fn get_profile(
        &self,
        user_id: String,
    ) -> Result<get_profile::v3::Response, anyhow::Error> {
        let parsed_id = ruma::OwnedUserId::try_from(user_id.clone())?;

        let profile = self
            .client
            .send_request(get_profile::v3::Request::new(parsed_id))
            .await?;

        Ok(profile)
    }

    pub async fn get_room_summary(
        &self,
        room_id: OwnedRoomId,
    ) -> Result<RoomSummary, anyhow::Error> {
        let mut room_info = RoomSummary {
            room_id: room_id.to_string(),
            ..Default::default()
        };

        let state = self
            .client
            .send_request(get_state_events::v3::Request::new(room_id))
            .await?;

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
                if let Ok(Some(content)) = state_event.get_field::<RoomNameEventContent>("content")
                {
                    room_info.name = Some(content.name.to_string());
                };
            }

            if event_type == "m.room.canonical_alias" {
                if let Ok(Some(content)) =
                    state_event.get_field::<RoomCanonicalAliasEventContent>("content")
                {
                    room_info.canonical_alias = content.alias.map(|a| a.to_string());
                };
            }

            if event_type == "m.room.avatar" {
                if let Ok(Some(content)) =
                    state_event.get_field::<RoomAvatarEventContent>("content")
                {
                    room_info.avatar_url = content.url.map(|u| u.to_string());
                };
            }

            if event_type == "commune.room.banner" {
                if let Ok(Some(content)) =
                    state_event.get_field::<RoomAvatarEventContent>("content")
                {
                    room_info.banner_url = content.url.map(|u| u.to_string());
                };
            }

            if event_type == "m.room.topic" {
                if let Ok(Some(content)) = state_event.get_field::<RoomTopicEventContent>("content")
                {
                    room_info.topic = Some(content.topic.to_string());
                };
            }
        }

        Ok(room_info)
    }

    pub async fn get_room_hierarchy(
        &self,
        room_id: OwnedRoomId,
    ) -> Result<Vec<SpaceHierarchyRoomsChunk>, anyhow::Error> {
        let hierarchy = self
            .client
            .send_request(get_hierarchy::v1::Request::new(room_id))
            .await?;

        Ok(hierarchy.rooms)
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
