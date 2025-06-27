use axum::{
    extract::{State, OriginalUri},
    http::{
        Request, 
        Response, 
        StatusCode, 
        HeaderMap
    },
    body::Body,
    Json,
    Extension,
};

use std::time::Duration;
use ruma::events::room::{
    member::{RoomMemberEvent, MembershipState},
    history_visibility::{RoomHistoryVisibilityEvent, HistoryVisibility},
};

use ruma::events::macros::EventContent;

use serde_json::{Value, json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use sha2::{Sha256, Digest};

use crate::AppState;
use crate::middleware::Data;

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
            return Ok(Json(json!({})))
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
                        info!("Joining room: {}", room_id);
                        let _ = state.appservice.join_room(room_id).await;
                    });

                    return Ok(Json(json!({})))
                }

            }
        };

        // Match commune.room.public types

        /*
        let room_id = event["room_id"].as_str();
        let event_type = event["type"].as_str();
        let public = event["content"]["public"].as_bool();

        match room_id {
            Some(room_id) => {
                tracing::info!("Room ID: {}", room_id);
                let room_id = RoomId::parse(room_id);
                 match (event_type, public) {
                    (Some("commune.room.public"), Some(true)) => {
                        info!("Joining room: {}", room_id);
                        let _ = state.appservice.join_room(room_id).await;
                    },
                    (Some("commune.room.public"), Some(false)) => {
                        tracing::info!("Leave room");

                    }
                    _ => {}
                }
            }
            None => {}
        }
        */

        let public = event["content"]["public"].as_bool();
        if let Ok(event) = serde_json::from_value::<CommunePublicRoomEvent>(event.clone()) {
            tracing::info!("Commune Public room event.");
            let room_id = event.room_id().to_owned();
            match public {
                Some(true) => {
                    info!("Joining room: {}", room_id);
                    let joined = state.appservice.join_room(room_id.clone()).await;
                    // cache the joined status
                    if let Ok(joined) = joined {
                        let cache_key = format!("appservice:joined:{}", room_id);
                        if let Ok(_) = state.cache.cache_data(&cache_key, &joined, 300).await {
                            tracing::info!("Cached joined status for room: {}", room_id);
                        } else {
                            tracing::warn!("Failed to cache joined status for room: {}", room_id);
                        
                        }
                    }
                }
                Some(false) => {
                    info!("Leaving room: {}", room_id);
                    let _ = state.appservice.leave_room(room_id.clone()).await;
                    let cache_key = format!("appservice:joined:{}", room_id);
                    let _ = state.cache.delete_cached_data(&cache_key).await;
                }
                None => {}
            }
        };


        let member_event = if let Ok(event) = serde_json::from_value::<RoomMemberEvent>(event.clone()) {
            event
        } else {
            continue;
        };

        print!("Member Event: {:#?}", member_event);

        let room_id = member_event.room_id().to_owned();
        let membership = member_event.membership().to_owned();
        let server_name = member_event.room_id().server_name();

        match server_name {
            Some(server_name) => {

                let allowed = state.config.appservice.rules.federation_domain_whitelist.iter().any(|domain| {
                    server_name.as_str().ends_with(domain)
                });


                if server_name.as_str() != state.config.matrix.server_name && allowed {
                    // Ignore events for rooms on other servers, if configured to local homeserver
                    // users
                    if state.config.appservice.rules.invite_by_local_user {
                        info!("Ignoring event for room on different server: {}", server_name);
                        continue;
                    }
                }
            }
            None => {
                info!("Ignoring event for room with no server name");
                continue;
            }
        }

        // Ignore membership events for other users
        let invited_user = member_event.state_key().to_owned();
        if invited_user != state.appservice.user_id() {
            info!("Ignoring event for user: {}", invited_user);
            continue;
        }

        match membership {
            MembershipState::Invite => {
                info!("Joining room: {}", room_id);
                let _ = state.appservice.join_room(room_id).await;
            }
            MembershipState::Leave => {
                let _ = state.appservice.leave_room(room_id).await;
            }
            MembershipState::Ban => {
                info!("Banned from room: {}", room_id);
                let _ = state.appservice.leave_room(room_id).await;
                //state.appservice.leave_room(room_id).await;
            }
            _ => {}
        }


    }


    Ok(Json(json!({})))
}

pub async fn matrix_proxy(
    Extension(data): Extension<Data>,
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
) -> Result<Response<Body>, StatusCode> {

    let is_media_request = data.is_media_request;

    let method = req.method().clone();
    let headers = req.headers().clone();
    
    let path = if let Some(mod_path) = data.modified_path.as_ref() {
        mod_path.as_str()
    } else if let Some(original_uri) = req.extensions().get::<OriginalUri>() {
        original_uri.0.path()
    } else {
        req.uri().path()
    };

    let mut target_url = format!("{}{}", state.config.matrix.homeserver, path);
    
    if data.modified_path.is_none() {
        if let Some(query) = req.uri().query() {
            target_url.push('?');
            target_url.push_str(query);
        }
    }


    let cache_key = format!("proxy_request:{}", target_url.clone());
    // check if response is cached and return it if so
    if state.config.cache.requests.enabled && !is_media_request {
        if let Ok(cached_response) = state.cache.get_cached_proxy_response(&cache_key).await {
            tracing::info!("Returning cached response for {}", target_url);

            if let Ok(response) = Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(axum::body::Body::from(cached_response))
                .map_err(|e| {
                    tracing::error!("Failed to build response: {}", e);
                }) {

                return Ok(response);
            }

        }
    }


    let body_bytes = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let mut request_builder = state.proxy
        .request(method, &target_url)
        .timeout(Duration::from_secs(25)) // Slightly less than overall timeout
        .bearer_auth(&state.config.appservice.access_token);

    let mut filtered_headers = HeaderMap::new();
    for (name, value) in headers.iter() {
        if !is_hop_by_hop_header(name.as_str()) && name != "authorization" {
            filtered_headers.insert(name, value.clone());
        }
    }

    request_builder = request_builder.headers(filtered_headers);

    if !body_bytes.is_empty() {
        request_builder = request_builder.body(body_bytes);
    }

    let response = request_builder.send()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let status = response.status();
    let headers = response.headers().clone();
    let body = response.bytes()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let to_cache = body.to_vec();
    let ttl = state.config.cache.requests.expire_after;

    if state.config.cache.requests.enabled && !is_media_request {
        tokio::spawn(async move {
            if let Ok(_) = state.cache.cache_proxy_response(
                &cache_key,
                &to_cache,
                ttl
            ).await {
                tracing::info!("Cached proxied response for {}", target_url);
            } else {
                tracing::warn!("Failed to cache proxied response for {}", target_url);
            }
        });
    }


    let mut axum_response = Response::builder().status(status);

    for (name, value) in headers.iter() {
        if !is_hop_by_hop_header(name.as_str()) {
            axum_response = axum_response.header(name, value);
        }
    }

    let response = axum_response
        .body(axum::body::Body::from(body))
        .map_err(|e| {
            tracing::error!("Failed to build response: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(response)
}


pub async fn matrix_proxy_search(
    Extension(data): Extension<Data>,
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
) -> Result<Response<Body>, StatusCode> {

    let method = req.method().clone();
    let headers = req.headers().clone();
    
    let path = if let Some(mod_path) = data.modified_path.as_ref() {
        mod_path.as_str()
    } else if let Some(original_uri) = req.extensions().get::<OriginalUri>() {
        original_uri.0.path()
    } else {
        req.uri().path()
    };

    let mut target_url = format!("{}{}", state.config.matrix.homeserver, path);
    
    if data.modified_path.is_none() {
        if let Some(query) = req.uri().query() {
            target_url.push('?');
            target_url.push_str(query);
        }
    }

    let body_bytes = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let cache_key = if state.config.cache.search.enabled {
        let mut hasher = Sha256::new();
        hasher.update(&body_bytes);
        let body_hash = format!("{:x}", hasher.finalize());
        format!("proxy_post_request:{}:{}", target_url, body_hash)
    } else {
        String::new() 
    };

    if state.config.cache.search.enabled {
        if let Ok(cached_response) = state.cache.get_cached_proxy_response(&cache_key).await {
            tracing::info!("Returning cached search response for {}", target_url);

            if let Ok(response) = Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(axum::body::Body::from(cached_response))
                .map_err(|e| {
                    tracing::error!("Failed to build response: {}", e);
                }) {

                return Ok(response);
            }

        }
    }

    let mut request_builder = state.proxy
        .request(method, &target_url)
        .timeout(Duration::from_secs(25)) // Slightly less than overall timeout
        .bearer_auth(&state.config.appservice.access_token);

    let mut filtered_headers = HeaderMap::new();
    for (name, value) in headers.iter() {
        if !is_hop_by_hop_header(name.as_str()) && name != "authorization" {
            filtered_headers.insert(name, value.clone());
        }
    }

    request_builder = request_builder.headers(filtered_headers);

    if !body_bytes.is_empty() {
        request_builder = request_builder.body(body_bytes);
    }

    let response = request_builder.send()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let status = response.status();
    let headers = response.headers().clone();
    let body = response.bytes()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;


    let to_cache = body.to_vec();
    let ttl = state.config.cache.search.expire_after;

    if state.config.cache.search.enabled {
        tokio::spawn(async move {
            if let Ok(_) = state.cache.cache_proxy_response(
                &cache_key,
                &to_cache,
                ttl
            ).await {
                tracing::info!("Cached proxied search response for {}", target_url);
            } else {
                tracing::warn!("Failed to cache search response for {}", target_url);
            }
        });
    }


    let mut axum_response = Response::builder().status(status);

    for (name, value) in headers.iter() {
        if !is_hop_by_hop_header(name.as_str()) {
            axum_response = axum_response.header(name, value);
        }
    }

    let response = axum_response
        .body(axum::body::Body::from(body))
        .map_err(|e| {
            tracing::error!("Failed to build response: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(response)
}


fn is_hop_by_hop_header(name: &str) -> bool {
    matches!(name.to_lowercase().as_str(),
        "connection" | "keep-alive" | "proxy-authenticate" | 
        "proxy-authorization" | "te" | "trailers" | "transfer-encoding" | "upgrade"
    )
}

