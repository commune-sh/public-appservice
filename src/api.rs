use axum::{Json, extract::State, http::StatusCode};

use ruma::events::room::{
    history_visibility::{HistoryVisibility, RoomHistoryVisibilityEvent},
    member::{MembershipState, RoomMemberEvent},
};
use ruma::events::space::child::SpaceChildEvent;
use std::time::Duration;

use ruma::events::macros::EventContent;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::AppState;

use crate::cache::CacheKey;

#[derive(Clone, Debug, Deserialize, Serialize, EventContent)]
#[ruma_event(type = "commune.public.room", kind = State, state_key_type = String)]
pub struct CommunePublicRoomEventContent {
    pub public: bool,
}

pub async fn transactions(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let events = match payload.get("events") {
        Some(Value::Array(events)) => events,
        Some(_) | None => {
            tracing::info!("Events is not an array");
            return Ok(Json(json!({})));
        }
    };

    for event in events {
        if cfg!(debug_assertions) {
            tracing::info!("Event: {:#?}", event);
        }

        // If auto-join is enabled, join rooms with world_readable history visibility
        if state.config.appservice.rules.auto_join {
            if let Ok(event) = serde_json::from_value::<RoomHistoryVisibilityEvent>(event.clone()) {
                if event.history_visibility() == &HistoryVisibility::WorldReadable {
                    tracing::info!("History Visibility: World Readable");

                    tokio::spawn(async move {
                        // Join the room if history visibility is world readable
                        // delay for a moment to allow the event to be processed
                        tokio::time::sleep(Duration::from_secs(5)).await;

                        let room_id = event.room_id().to_owned();
                        tracing::info!("Joining room: {}", room_id);
                        if let Err(e) = state.appservice.join_room(&room_id).await {
                            tracing::warn!("Failed to join room: {}. Error: {}", room_id, e);
                        } else {
                            tracing::info!("Successfully joined room: {}", room_id);
                        }
                    });

                    return Ok(Json(json!({})));
                }
            }

            if let Ok(event) = serde_json::from_value::<SpaceChildEvent>(event.clone()) {
                tracing::info!("Auto joining space child room");

                tokio::spawn(async move {
                    let room_id = event.room_id().to_owned();
                    tracing::info!("Joining room: {}", room_id);
                    if let Err(e) = state.appservice.join_room(&room_id).await {
                        tracing::warn!("Failed to join room: {}. Error: {}", room_id, e);
                    } else {
                        tracing::info!("Successfully joined room: {}", room_id);
                    }
                });

                return Ok(Json(json!({})));
            }
        };

        let public = event["content"]["public"].as_bool();
        if let Ok(event) = serde_json::from_value::<CommunePublicRoomEvent>(event.clone()) {
            tracing::info!("Commune Public room event.");
            let room_id = event.room_id().to_owned();
            match public {
                Some(true) => {
                    tracing::info!("Joining room: {}", room_id);
                    let joined = state.appservice.join_room(&room_id).await;
                    // cache the joined status
                    if let Ok(joined) = joined {
                        let cache_key = ("appservice:joined", room_id.as_str()).cache_key();
                        if (state.cache.cache_data(&cache_key, &joined, 300).await).is_ok() {
                            tracing::info!("Cached joined status for room: {}", room_id);
                        } else {
                            tracing::warn!("Failed to cache joined status for room: {}", room_id);
                        }
                    }
                }
                Some(false) => {
                    tracing::info!("Leaving room: {}", room_id);
                    if let Err(e) = state.appservice.leave_room(&room_id).await {
                        tracing::warn!("Failed to leave room: {}. Error: {}", room_id, e);
                    } else {
                        tracing::info!("Successfully left room: {}", room_id);
                    }
                    let cache_key = ("appservice:joined", room_id.as_str()).cache_key();
                    if let Err(e) = state.cache.delete_cached_data(&cache_key).await {
                        tracing::warn!(
                            "Failed to delete room from cache: {}. Error: {}",
                            room_id,
                            e
                        );
                    } else {
                        tracing::info!("Successfully removed room from cache: {}", room_id);
                    }
                }
                None => {}
            }
        };

        let member_event =
            if let Ok(event) = serde_json::from_value::<RoomMemberEvent>(event.clone()) {
                event
            } else {
                continue;
            };

        println!("Member Event: {member_event:#?}");

        let room_id = member_event.room_id().to_owned();
        let membership = member_event.membership().to_owned();
        let server_name = member_event.room_id().server_name();

        match server_name {
            Some(server_name) => {
                let allowed = state
                    .config
                    .appservice
                    .rules
                    .federation_domain_whitelist
                    .iter()
                    .any(|domain| server_name.as_str().ends_with(domain));

                if server_name.as_str() != state.config.matrix.server_name && allowed {
                    // Ignore events for rooms on other servers, if configured to local homeserver
                    // users
                    if state.config.appservice.rules.invite_by_local_user {
                        tracing::info!(
                            "Ignoring event for room on different server: {}",
                            server_name
                        );
                        continue;
                    }
                }
            }
            None => {
                tracing::info!("Ignoring event for room with no server name");
                continue;
            }
        }

        // Ignore membership events for other users
        let invited_user = member_event.state_key().to_owned();
        if invited_user != state.appservice.user_id() {
            tracing::info!("Ignoring event for user: {}", invited_user);
            continue;
        }

        match membership {
            MembershipState::Invite => {
                tracing::info!("Joining room: {}", room_id);
                if let Err(e) = state.appservice.join_room(&room_id).await {
                    tracing::warn!("Failed to join room: {}. Error: {}", room_id, e);
                } else {
                    tracing::info!("Successfully joined room: {}", room_id);
                }
                if state
                    .appservice
                    .add_to_joined_rooms(room_id.clone())
                    .is_err()
                {
                    tracing::warn!("Failed to add room to joined rooms list: {}", room_id);
                }
            }
            MembershipState::Leave => {
                if let Err(e) = state.appservice.leave_room(&room_id).await {
                    tracing::warn!("Failed to leave room: {}. Error: {}", room_id, e);
                } else {
                    tracing::info!("Successfully left room: {}", room_id);
                }
                if let Err(e) = state.appservice.remove_from_joined_rooms(&room_id) {
                    tracing::warn!(
                        "Failed to remove room from joined rooms list: {} {}",
                        room_id,
                        e
                    );
                }
            }
            MembershipState::Ban => {
                tracing::info!("Banned from room: {}", room_id);
                if let Err(e) = state.appservice.remove_from_joined_rooms(&room_id) {
                    tracing::warn!(
                        "Failed to remove room from joined rooms list: {} {}",
                        room_id,
                        e
                    );
                }
            }
            _ => {}
        }
    }

    Ok(Json(json!({})))
}
