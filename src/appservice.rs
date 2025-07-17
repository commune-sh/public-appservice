use crate::config::Config;
use futures::future::join_all;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Semaphore;

use ruma::{
    OwnedEventId, OwnedRoomId, OwnedTransactionId, OwnedUserId, RoomAliasId, UserId,
    api::Direction,
    api::client::{
        account::whoami,
        alias::get_alias,
        appservice::request_ping,
        membership::{join_room_by_id, joined_rooms, leave_room},
        message::get_message_events,
        profile::get_profile,
        room::get_room_event,
        space::{SpaceHierarchyRoomsChunk, get_hierarchy},
        state::{get_state_events, get_state_events_for_key},
    },
    events::{
        AnyStateEvent, AnyTimelineEvent, StateEventType,
        room::{
            avatar::RoomAvatarEventContent, canonical_alias::RoomCanonicalAliasEventContent,
            name::RoomNameEventContent, topic::RoomTopicEventContent,
        },
    },
};

use anyhow;

use serde::{Deserialize, Serialize};

pub type HttpClient = ruma_client::http_client::Reqwest;

use std::sync::Mutex;

use crate::rooms::CommuneRoomType;

#[derive(Clone)]
pub struct AppService {
    client: ruma::Client<HttpClient>,
    config: Config,
    pub appservice_id: String,
    pub user_id: Box<OwnedUserId>,
    pub joined_rooms: Arc<Mutex<Vec<OwnedRoomId>>>,
}

pub type RoomState = Vec<ruma::serde::Raw<AnyStateEvent>>;

#[derive(Clone)]
pub struct JoinedRoomState {
    pub room_id: OwnedRoomId,
    pub state: Option<RoomState>,
}

impl AppService {
    pub async fn new(config: &Config) -> Result<Self, anyhow::Error> {
        let reqwest_client = ruma_client::http_client::Reqwest::builder()
            .user_agent("commune-public-appservice")
            .build()?;

        let client = ruma_client::Client::builder()
            .homeserver_url(config.matrix.homeserver.clone())
            .access_token(Some(config.appservice.access_token.clone()))
            .http_client(reqwest_client)
            .await?;

        let user_id = UserId::parse(format!(
            "@{}:{}",
            config.appservice.sender_localpart, config.matrix.server_name
        ))?;

        let whoami = client.send_request(whoami::v3::Request::new()).await;

        if whoami.is_err() {
            tracing::info!("Failed to authenticate with homeserver. Check your access token.");
            return Err(anyhow::anyhow!(
                "Failed to authenticate with homeserver. Check your appservice access token."
            ));
        }

        let joined_rooms = match client.send_request(joined_rooms::v3::Request::new()).await {
            Ok(r) => r.joined_rooms,
            Err(_) => vec![],
        };

        Ok(Self {
            client,
            config: config.clone(),
            appservice_id: config.appservice.id.clone(),
            user_id: Box::new(user_id),
            joined_rooms: Arc::new(Mutex::new(joined_rooms)),
        })
    }

    pub fn add_to_joined_rooms(&self, room_id: OwnedRoomId) -> Result<(), anyhow::Error> {
        let mut rooms = self
            .joined_rooms
            .lock()
            .map_err(|_| anyhow::anyhow!("Failed to acquire lock on joined_rooms"))?;

        if !rooms.contains(&room_id) {
            rooms.push(room_id);
        }
        Ok(())
    }

    pub fn remove_from_joined_rooms(&self, room_id: &OwnedRoomId) -> Result<(), anyhow::Error> {
        let mut rooms = self
            .joined_rooms
            .lock()
            .map_err(|_| anyhow::anyhow!("Failed to acquire lock on joined_rooms"))?;

        if let Some(pos) = rooms.iter().position(|x| x == room_id) {
            rooms.remove(pos);
        }
        tracing::info!(
            "Removed room {} from joined rooms. Current count: {}",
            room_id,
            rooms.len()
        );
        Ok(())
    }

    pub async fn health_check(&self) -> Result<(), anyhow::Error> {
        // Perform a simple request to check if the appservice is healthy
        let response = self.client.send_request(whoami::v3::Request::new()).await?;

        if response.user_id == *self.user_id {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Health check failed: User ID mismatch"))
        }
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

    pub async fn join_room(&self, room_id: &OwnedRoomId) -> Result<bool, anyhow::Error> {
        let jr = self
            .client
            .send_request(join_room_by_id::v3::Request::new(room_id.clone()))
            .await?;

        tracing::info!("Joined room: {:#?}", jr);

        Ok(jr.room_id == *room_id)
    }

    pub async fn has_joined_room(&self, room_id: &OwnedRoomId) -> Result<bool, anyhow::Error> {
        let jr = self
            .client
            .send_request(get_state_events_for_key::v3::Request::new(
                room_id.clone(),
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

    pub async fn is_space(&self, room_id: OwnedRoomId) -> Result<bool, anyhow::Error> {
        let hierarchy = self
            .client
            .send_request(get_hierarchy::v1::Request::new(room_id.clone()))
            .await?;

        // hierarchy contains the room itself and all child rooms
        // so we check if there is more than one room in the hierarchy
        Ok(hierarchy.rooms.len() > 1)
    }

    pub async fn leave_room(&self, room_id: &OwnedRoomId) -> Result<(), anyhow::Error> {
        // First leave all child rooms
        let hierarchy = self
            .client
            .send_request(get_hierarchy::v1::Request::new(room_id.clone()))
            .await?;

        tracing::info!("Hierarchy rooms: {:#?}", hierarchy.rooms.len());

        for room in hierarchy.rooms {
            if room.room_id == *room_id {
                continue;
            }
            let left = self
                .client
                .send_request(leave_room::v3::Request::new(room.room_id.clone()))
                .await?;
            tracing::info!("Left child room: {:#?}", room.room_id);
            tracing::info!("Left child room: {:#?}", left);
        }

        let left = self
            .client
            .send_request(leave_room::v3::Request::new(room_id.clone()))
            .await?;

        tracing::info!("Left room: {:#?}", left);

        Ok(())
    }

    pub async fn joined_rooms(&self) -> Result<Vec<OwnedRoomId>, anyhow::Error> {
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
        let semaphore = Arc::new(Semaphore::new(10));
        let curated = self.config.public_rooms.curated;
        let include_rooms = &self.config.public_rooms.include_rooms;

        if curated && !include_rooms.is_empty() {
            let mut all_room_ids = Vec::new();

            let space_futures: Vec<_> = include_rooms
                .iter()
                .map(|local_part| {
                    let sem = semaphore.clone();
                    let server_name = &self.config.matrix.server_name;
                    let self_ref = self;
                    async move {
                        let _permit = sem.acquire().await.ok()?;

                        let alias = match local_part.contains(':') && local_part.contains('.') {
                            true => {
                                if local_part.starts_with('#') {
                                    local_part.to_string()
                                } else {
                                    format!("#{local_part}")
                                }
                            }
                            false => format!("#{local_part}:{server_name}"),
                        };

                        let alias = RoomAliasId::parse(&alias).ok()?;
                        let room_id = self_ref.room_id_from_alias(alias).await.ok()?;

                        // Get hierarchy for this space
                        let hierarchy = self_ref
                            .client
                            .send_request(get_hierarchy::v1::Request::new(room_id.clone()))
                            .await
                            .ok()?;

                        let mut room_ids = vec![room_id];
                        room_ids.extend(hierarchy.rooms.into_iter().map(|room| room.room_id));

                        Some(room_ids)
                    }
                })
                .collect();

            let hierarchy_results = join_all(space_futures).await;

            let mut unique_room_ids = HashSet::new();
            for room_ids in hierarchy_results.into_iter().flatten() {
                for room_id in room_ids {
                    unique_room_ids.insert(room_id);
                }
            }
            all_room_ids.extend(unique_room_ids);

            let state_futures: Vec<_> = all_room_ids
                .into_iter()
                .map(|room_id| {
                    let sem = semaphore.clone();
                    let self_ref = self;
                    async move {
                        let _permit = sem.acquire().await.ok()?;

                        let st = self_ref
                            .client
                            .send_request(get_state_events::v3::Request::new(room_id.clone()))
                            .await
                            .ok()?;

                        Some(JoinedRoomState {
                            room_id,
                            state: Some(st.room_state),
                        })
                    }
                })
                .collect();

            let state_results = join_all(state_futures).await;
            let joined_rooms: Vec<_> = state_results.into_iter().flatten().collect();

            return Ok(Some(joined_rooms));
        }

        // Handle the non-curated case
        let jr = self
            .client
            .send_request(joined_rooms::v3::Request::new())
            .await?;

        if jr.joined_rooms.is_empty() {
            return Ok(None);
        }

        let state_futures: Vec<_> = jr
            .joined_rooms
            .into_iter()
            .map(|room_id| {
                let sem = semaphore.clone();
                let self_ref = self;
                async move {
                    let _permit = sem.acquire().await.ok()?;

                    let st = self_ref
                        .client
                        .send_request(get_state_events::v3::Request::new(room_id.clone()))
                        .await
                        .ok()?;

                    Some(JoinedRoomState {
                        room_id,
                        state: Some(st.room_state),
                    })
                }
            })
            .collect();

        let results = join_all(state_futures).await;
        let joined_rooms: Vec<_> = results.into_iter().flatten().collect();

        Ok(Some(joined_rooms))
    }

    pub async fn joined_rooms_state_alt(
        &self,
    ) -> Result<Option<Vec<JoinedRoomState>>, anyhow::Error> {
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
        user_id: &str,
    ) -> Result<get_profile::v3::Response, anyhow::Error> {
        let parsed_id = ruma::OwnedUserId::try_from(user_id.to_string())?;

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
        // find out if appservice has joined the room or not
        let has_joined = self.has_joined_room(&room_id).await?;

        if !has_joined {
            // If not joined, we cannot get the state
            return Err(anyhow::anyhow!(
                "Appservice has not joined the room: {}",
                room_id
            ));
        }

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
                _ => continue,
            };

            match event_type.as_str() {
                "m.room.name" => {
                    if let Ok(Some(content)) =
                        state_event.get_field::<RoomNameEventContent>("content")
                    {
                        room_info.name = if content.name.is_empty() {
                            None
                        } else {
                            Some(content.name.to_string())
                        };
                    }
                }
                "m.room.canonical_alias" => {
                    if let Ok(Some(content)) =
                        state_event.get_field::<RoomCanonicalAliasEventContent>("content")
                    {
                        room_info.canonical_alias = content.alias.map(|a| a.to_string());
                    }
                }
                "m.room.avatar" => {
                    if let Ok(Some(content)) =
                        state_event.get_field::<RoomAvatarEventContent>("content")
                    {
                        room_info.avatar_url = match content.url {
                            Some(url) => {
                                if url.to_string().is_empty() {
                                    None
                                } else {
                                    Some(url.to_string())
                                }
                            }
                            None => None,
                        };
                    }
                }
                "commune.room.banner" => {
                    if let Ok(Some(content)) =
                        state_event.get_field::<RoomAvatarEventContent>("content")
                    {
                        room_info.banner_url = content.url.map(|url| url.to_string());
                    }
                }
                "m.room.topic" => {
                    if let Ok(Some(content)) =
                        state_event.get_field::<RoomTopicEventContent>("content")
                    {
                        room_info.topic = if content.topic.is_empty() {
                            None
                        } else {
                            Some(content.topic.to_string())
                        };
                    }
                }
                "commune.room.type" => {
                    if let Ok(Some(content)) = state_event.get_field::<CommuneRoomType>("content") {
                        match content.room_type.map(|t| t.to_string()) {
                            Some(t) if t == "chat" => room_info.room_type = RoomType::Chat,
                            Some(t) if t == "forum" => room_info.room_type = RoomType::Forum,
                            _ => room_info.room_type = RoomType::Chat, // Default to Chat
                        }
                    }
                }
                _ => {}
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
            .send_request(get_hierarchy::v1::Request::new(room_id.clone()))
            .await?;

        Ok(hierarchy.rooms)
    }

    pub async fn get_space_rooms(
        &self,
        room_id: OwnedRoomId,
    ) -> Result<Vec<RoomSummary>, anyhow::Error> {
        let mut hierarchy = self
            .client
            .send_request(get_hierarchy::v1::Request::new(room_id.clone()))
            .await?;

        let mut room_summaries: Vec<RoomSummary> = Vec::new();

        //remove the room itself from the hierarchy
        hierarchy.rooms.retain(|room| room.room_id != room_id);

        // build room summaries for each room in the hierarchy
        let semaphore = Arc::new(Semaphore::new(10));
        let room_futures: Vec<_> = hierarchy
            .rooms
            .into_iter()
            .map(|room| {
                let sem = semaphore.clone();
                let self_ref = self.clone();
                async move {
                    let _permit = sem.acquire().await.ok()?;

                    let summary = self_ref.get_room_summary(room.room_id).await.ok()?;
                    Some(summary)
                }
            })
            .collect();

        let results = join_all(room_futures).await;
        for result in results.into_iter().flatten() {
            room_summaries.push(result);
        }

        Ok(room_summaries)
    }

    pub async fn get_public_spaces(&self) -> Result<Option<Vec<RoomSummary>>, anyhow::Error> {
        let semaphore = Arc::new(Semaphore::new(10));

        if self.config.spaces.include_all {
            let jr = self
                .client
                .send_request(joined_rooms::v3::Request::new())
                .await?;

            if jr.joined_rooms.is_empty() {
                return Ok(None);
            }

            let space_futures: Vec<_> = jr
                .joined_rooms
                .into_iter()
                .map(|room_id| {
                    let sem = semaphore.clone();
                    let self_ref = self;
                    async move {
                        let _permit = sem.acquire().await.ok()?;

                        let is_space = self_ref.is_space(room_id.clone()).await.ok()?;
                        if !is_space {
                            return None;
                        }

                        let summary = self_ref.get_room_summary(room_id).await.ok()?;
                        Some(summary)
                    }
                })
                .collect();

            let results = join_all(space_futures).await;
            let spaces: Vec<_> = results.into_iter().flatten().collect();

            if spaces.is_empty() {
                return Ok(None);
            }

            return Ok(Some(spaces));
        }

        let default_spaces = self.config.spaces.default.clone();

        if default_spaces.is_empty() {
            return Ok(None);
        }

        let space_futures: Vec<_> = default_spaces
            .into_iter()
            .map(|space| {
                let sem = semaphore.clone();
                let server_name = &self.config.matrix.server_name;
                let self_ref = self;
                async move {
                    let _permit = sem.acquire().await.ok()?;

                    let alias = match space.contains(':') && space.contains('.') {
                        true => {
                            if space.starts_with('#') {
                                space.to_string()
                            } else {
                                format!("#{space}")
                            }
                        }
                        false => format!("#{space}:{server_name}"),
                    };

                    let alias = RoomAliasId::parse(&alias).ok()?;

                    let room_id = self_ref.room_id_from_alias(alias).await.ok()?;

                    let summary = self_ref.get_room_summary(room_id).await.ok()?;
                    Some(summary)
                }
            })
            .collect();

        let results = join_all(space_futures).await;
        let spaces: Vec<_> = results.into_iter().flatten().collect();

        Ok(Some(spaces))
    }

    pub async fn get_room_messages(
        &self,
        room_id: OwnedRoomId,
    ) -> Result<get_message_events::v3::Response, anyhow::Error> {
        let dir = Direction::Backward;

        let mut req = get_message_events::v3::Request::new(room_id, dir);

        let limit =
            ruma::UInt::try_from(100).map_err(|_| anyhow::anyhow!("Invalid limit value"))?;

        req.limit = limit;

        let response = self.client.send_request(req).await?;

        Ok(response)
    }
}

#[derive(Default, Clone, Debug, Deserialize, Serialize)]
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
    pub room_type: RoomType,
}

#[derive(Default, Clone, Debug, Deserialize, Serialize)]
pub enum RoomType {
    #[default]
    #[serde(rename = "chat")]
    Chat,
    #[serde(rename = "forum")]
    Forum,
}
