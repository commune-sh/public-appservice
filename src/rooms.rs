use axum::{
    extract::{State },
    http::{StatusCode },
    response::IntoResponse,
    Json,
};

use serde_json::{json, Value};

use std::sync::Arc;

use crate::server::AppState;


use ruma::{
    events::room::create::RoomCreateEvent,
};



type RoomState = crate::appservice::RoomState;

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

fn process_rooms(rooms: Vec<RoomState>) -> Option<Vec<RoomState>> {

    let mut items = Vec::new();


    for room in rooms {
        let mut room_state = Vec::new();

        for state_event in room {

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
                    Ok(event) => println!("Event: {:#?}", event),
                    Err(_) => () // Silently ignore deserialization errors
                }

                room_state.push(state_event);
            }


        }

        items.push(room_state);
    }

    Some(items)
}
