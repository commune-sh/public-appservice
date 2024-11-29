use axum::{
    extract::{State, OriginalUri},
    http::{
        Request, 
        Response, 
        StatusCode, 
        Uri, 
        HeaderValue, 
        header::AUTHORIZATION
    },
    response::IntoResponse,
    body::Body,
    Json,
    Extension,
};

use ruma::events::room::member::{RoomMemberEvent, MembershipState};

use serde_json::{Value, json};
use std::sync::Arc;
use tracing::info;

use crate::server::AppState;
use crate::middleware::Data;

pub async fn transactions(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {

    let events = match payload.get("events") {
        Some(Value::Array(events)) => events,
        Some(_) | None => {
            println!("Events is not an array");
            return Ok(Json(json!({})))
        }
    };


    for event in events {
        println!("Event: {:#?}", event);

        if let Ok(event) =  serde_json::from_value::<RoomMemberEvent>(event.clone()) {
            let room_id = event.room_id().to_owned();

            let membership = event.membership().to_owned();

            let server_name = event.room_id().server_name();

            match server_name {
                Some(server_name) => {
                    if server_name.as_str() != state.config.matrix.server_name {
                        println!("Ignoring event for room on different server: {}", server_name);
                        continue;
                    }
                }
                None => {
                    println!("Ignoring event for room with no server name");
                    continue;
                }
            }


            // Ignore membership events for other users
            let invited_user = event.state_key().to_owned();
            if invited_user != state.appservice.user_id() {
                info!("Ignoring event for user: {}", invited_user);
                continue;
            }

            match membership {
                MembershipState::Leave => {
                    println!("Leaving room: {}", room_id);
                }
                MembershipState::Ban => {
                    println!("Banning user from room: {}", room_id);
                    state.appservice.leave_room(room_id).await;
                }
                MembershipState::Invite => {
                    println!("Joining room: {}", room_id);
                    state.appservice.join_room(room_id).await;
                }
                _ => {}
            }


        }
    }


    Ok(Json(json!({})))
}


pub async fn matrix_proxy(
    Extension(data): Extension<Data>,
    State(state): State<Arc<AppState>>,
    mut req: Request<Body>,
) -> Result<Response<Body>, StatusCode> {

    let mut path = if let Some(path) = req.extensions().get::<OriginalUri>() {
        path.0.path()
    } else {
        req.uri().path()
    };

    if let Some(mod_path) = data.modified_path.as_ref() {
        path = mod_path;
    }

    let path_query = req.uri().query().map(|q| format!("?{}", q)).unwrap_or_default();

    let homeserver = &state.config.matrix.homeserver;

    let uri = format!("{}{}{}", homeserver, path, path_query);

    *req.uri_mut() = Uri::try_from(uri).unwrap();

    let access_token = &state.config.appservice.access_token;

    let auth_value = HeaderValue::from_str(&format!("Bearer {}", access_token))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    req.headers_mut().insert(AUTHORIZATION, auth_value);

    Ok(state.client
        .request(req)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .into_response())
}
