use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};

use ruma::{
    EventId, RoomId,
    events::AnyTimelineEvent,
};

use serde_json::json;
use serde::{Deserialize, Serialize};

use std::sync::Arc;

use crate::AppState;

use crate::middleware::Data;

use crate::utils;

use crate::error::AppserviceError;

#[derive(Debug, Deserialize, Serialize)]
pub struct RoomInfoParams {
    pub room: Option<String>,
    pub event: Option<String>,
}

use crate::appservice::RoomSummary;

#[derive(Debug, Deserialize, Serialize)]
pub struct RoomInfo {
    #[serde(skip_deserializing)]
    info: RoomSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    room: Option<RoomSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    event: Option<ruma::serde::Raw<AnyTimelineEvent>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sender: Option<Sender>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Sender {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub displayname: Option<String>,
}
pub async fn board_events(
    Path(params): Path<Vec<(String, String)>>,
    Extension(data): Extension<Data>,
    Query(query): Query<RoomInfoParams>,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppserviceError> {
    let mut room_id = params[0].1.clone();

    if let Some(id) = data.room_id.as_ref() {
        room_id = id.to_string();
    }

    let mut parsed_id = RoomId::parse(&room_id).map_err(|e| {
        tracing::error!("Invalid room ID: {}", &room_id);
        AppserviceError::MatrixError(format!("Invalid room ID: {e}"))
    })?;

    let summary = state
        .appservice
        .get_room_summary(parsed_id.clone())
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch room summary for {}: {}", parsed_id, e);
            AppserviceError::MatrixError("Room not found".to_string())
        })?;

    let mut info = RoomInfo {
        info: summary,
        room: None,
        event: None,
        sender: None,
    };

    if let Some(alias) = query.room {
        let hierarchy = state
            .appservice
            .get_room_hierarchy(parsed_id.clone())
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch room hierarchy for {}: {}", parsed_id, e);
                AppserviceError::MatrixError("Failed to fetch room hierarchy".to_string())
            })?;

        for room in hierarchy {
            if let Some(name) = room.summary.name.as_ref() {
                let slug = utils::slugify(name);

                if slug == alias {
                    parsed_id = room.summary.room_id.clone();

                    let summary = state
                        .appservice
                        .get_room_summary(parsed_id.clone())
                        .await
                        .map_err(|e| {
                            tracing::error!(
                                "Failed to fetch room summary for {}: {}",
                                parsed_id,
                                e
                            );
                            AppserviceError::MatrixError("Room not found".to_string())
                        })?;

                    info.room = Some(summary);
                    break;
                }
            }
        }
    }

    if let Some(event_id) = query.event {
        let parsed_event_id = EventId::parse(&event_id).map_err(|e| {
            tracing::error!("Invalid event ID: {}", &event_id);
            AppserviceError::MatrixError(format!("Invalid event ID: {e}"))
        })?;

        let event = state
            .appservice
            .get_room_event(parsed_id, parsed_event_id)
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch event {}: {}", event_id, e);
                AppserviceError::MatrixError("Event not found".to_string())
            })?;

        info.event = Some(event.clone());

        if let Ok(Some(sender)) = event.get_field::<String>("sender") {
            tracing::info!("sender: {:#?}", sender);

            let profile = state.appservice.get_profile(&sender).await.map_err(|e| {
                tracing::error!("Failed to fetch profile for {}: {}", sender, e);
                AppserviceError::MatrixError("Failed to fetch sender profile".to_string())
            })?;

            let avatar_url = profile.get("avatar_url").and_then(|v| v.as_str()).map(|s| s.to_string());
            let displayname = profile.get("displayname").and_then(|v| v.as_str()).map(|s| s.to_string());

            info.sender = Some(Sender {
                avatar_url: avatar_url,
                displayname: displayname,
            });
        }
    }

    Ok((StatusCode::OK, Json(json!(info))))
}

