use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};

use serde::Serialize;

use serde_json::{json, Value};

use std::sync::Arc;

use crate::server::AppState;


use ruma::events::room::create::RoomCreateEvent;


use crate::appservice::JoinedRoomState;


pub async fn public_rooms (
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<Value>)> {

    let rooms = state.appservice.joined_rooms_state().await;

    if let Some(room_states) = rooms {

        let processed = process_rooms(room_states);

        return Ok((
            StatusCode::OK,
            Json(json!({
                "rooms": json!(processed),
            }))
        ))
    }

    Ok((
        StatusCode::OK,
        Json(json!({
            "rooms": json!([]),
        }))
    ))
}

#[derive(Default, Serialize)]
struct PublicRoom {
    room_id: String,
    #[serde(rename = "type")]
    room_type: Option<String>,
    origin_server_ts: Option<u64>,
    #[serde(rename = "room_type")]
    #[serde(skip_serializing_if = "Option::is_none")]
    commune_room_type: Option<String>,
    name: Option<String>,
    canonical_alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    commune_alias: Option<String>,
    sender: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    avatar_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    banner_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    topic: Option<String>,
    join_rule: Option<String>,
    history_visibility: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parents: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    settings: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    room_categories: Option<Vec<Value>>,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_false")]
    is_bridge: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn process_rooms(rooms: Vec<JoinedRoomState>) -> Option<Vec<PublicRoom>> {

    let mut public_rooms: Vec<PublicRoom> = Vec::new();

    for room in rooms {

        let mut pub_room = PublicRoom {
            room_id: room.room_id.to_string(),
            ..Default::default()
        };

        for state_event in room.state.unwrap_or_else(|| Vec::new()) {

            let event_type = match state_event.get_field::<String>("type") {
                Ok(Some(t)) => t,
                Ok(None) => {
                    continue;
                }
                Err(_) => {
                    continue;
                }
            };

            println!("Event type: {}", event_type);


            if event_type == "m.room.create" {
                // Method 1a: Using match
                match state_event.deserialize_as::<RoomCreateEvent>() {
                    Ok(event) => {
                    }
                    Err(_) => () // Silently ignore deserialization errors
                }

            }

        }

        public_rooms.push(pub_room);
    }

    Some(public_rooms)
}
