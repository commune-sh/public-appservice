use axum::{
    Extension, Json,
    body::Body,
    extract::{OriginalUri, State},
    http::{HeaderMap, Request, Response, StatusCode},
};

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

use sha2::{Digest, Sha256};

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
                        if let Err(e) = state.appservice.join_room(room_id.clone()).await {
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
                    if let Err(e) = state.appservice.join_room(room_id.clone()).await {
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
                    tracing::info!("Leaving room: {}", room_id);
                    if let Err(e) = state.appservice.leave_room(room_id.clone()).await {
                        tracing::warn!("Failed to leave room: {}. Error: {}", room_id, e);
                    } else {
                        tracing::info!("Successfully left room: {}", room_id);
                    }
                    let cache_key = format!("appservice:joined:{}", room_id);
                    if let Err(e) = state.cache.delete_cached_data(&cache_key).await {
                        tracing::warn!("Failed to delete room from cache: {}. Error: {}", room_id, e);
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

        print!("Member Event: {:#?}", member_event);

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
                if let Err(e) = state.appservice.join_room(room_id.clone()).await {
                    tracing::warn!("Failed to join room: {}. Error: {}", room_id, e);
                } else {
                    tracing::info!("Successfully joined room: {}", room_id);
                }
                if let Err(_) = state.appservice.add_to_joined_rooms(room_id.clone()) {
                    tracing::warn!("Failed to add room to joined rooms list: {}", room_id);
                }
            }
            MembershipState::Leave => {
                if let Err(e) = state.appservice.leave_room(room_id.clone()).await {
                    tracing::warn!("Failed to leave room: {}. Error: {}", room_id, e);
                } else {
                    tracing::info!("Successfully left room: {}", room_id);
                }
                if let Err(e) = state.appservice.remove_from_joined_rooms(&room_id) {
                    tracing::warn!("Failed to remove room from joined rooms list: {} {}", room_id, e);
                }
            }
            MembershipState::Ban => {
                tracing::info!("Banned from room: {}", room_id);
                if let Err(e) = state.appservice.remove_from_joined_rooms(&room_id) {
                    tracing::warn!("Failed to remove room from joined rooms list: {} {}", room_id, e);
                }
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

    let cache_key = format!("proxy_request:{}", target_url);

    // only cache non-media requests
    if !state.config.cache.requests.enabled || is_media_request {
        return proxy_request_no_cache(state, method, headers, target_url, req).await;
    }

    if let Ok(Some(cached_response)) = state.cache.get_cached_data::<Vec<u8>>(&cache_key).await {
        tracing::info!("Returning cached proxy response for {} ({} bytes)", target_url, cached_response.len());

        return Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(axum::body::Body::from(cached_response))
            .map_err(|e| {
                tracing::error!("Failed to build cached response: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            });
    }

    // cache missed
    let response_data = state.cache
        .cache_or_fetch(
            &cache_key,
            state.config.cache.requests.ttl,
            || async {
                tracing::info!("Cache miss for proxy request: {}", target_url);

                let body_bytes = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
                    Ok(bytes) => bytes,
                    Err(_) => {
                        return Err(redis::RedisError::from((
                            redis::ErrorKind::IoError,
                            "Failed to read request body",
                        )));
                    }
                };

                let mut request_builder = state
                    .proxy
                    .request(method.clone(), &target_url)
                    .timeout(Duration::from_secs(25))
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

                let response = request_builder
                    .send()
                    .await
                    .map_err(|e| {
                        tracing::error!("Proxy request failed for {}: {}", target_url, e);
                        redis::RedisError::from((
                            redis::ErrorKind::IoError,
                            "Proxy request failed",
                        ))
                    })?;

                let body = response
                    .bytes()
                    .await
                    .map_err(|e| {
                        tracing::error!("Failed to read proxy response body for {}: {}", target_url, e);
                        redis::RedisError::from((
                            redis::ErrorKind::IoError,
                            "Failed to read response body",
                        ))
                    })?;

                let response_vec = body.to_vec();
                tracing::info!("Fetched and cached proxy response for {} ({} bytes)", target_url, response_vec.len());

                Ok(response_vec)
            }
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to get proxy response for {}: {}", target_url, e);
            StatusCode::BAD_GATEWAY
        })?;


    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(axum::body::Body::from(response_data))
        .map_err(|e| {
            tracing::error!("Failed to build response: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

async fn proxy_request_no_cache(
    state: Arc<AppState>,
    method: axum::http::Method,
    headers: HeaderMap,
    target_url: String,
    req: Request<Body>,
) -> Result<Response<Body>, StatusCode> {
    let body_bytes = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let mut request_builder = state
        .proxy
        .request(method, &target_url)
        .timeout(Duration::from_secs(25))
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

    let response = request_builder
        .send()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    let status = response.status();
    let response_headers = response.headers().clone();
    let body = response
        .bytes()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut axum_response = Response::builder().status(status);

    for (name, value) in response_headers.iter() {
        if !is_hop_by_hop_header(name.as_str()) {
            axum_response = axum_response.header(name, value);
        }
    }

    axum_response
        .body(axum::body::Body::from(body))
        .map_err(|e| {
            tracing::error!("Failed to build response: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
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
        Err(e) => {
            tracing::error!("Failed to read request body for {}: {}", &target_url, e);
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
                })
            {
                return Ok(response);
            }
        }
    }

    let mut request_builder = state
        .proxy
        .request(method, &target_url)
        .timeout(Duration::from_secs(25)) 
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

    let response = request_builder
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to build request for {}: {}", target_url, e);
            StatusCode::BAD_GATEWAY
        })?;

    let status = response.status();
    let headers = response.headers().clone();
    let body = response
        .bytes()
        .await
        .map_err(|e| {
            tracing::error!("Failed to read response body for {}: {}", target_url, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let to_cache = body.to_vec();
    let ttl = state.config.cache.search.ttl;

    if state.config.cache.search.enabled {
        tokio::spawn(async move {
            if let Ok(_) = state
                .cache
                .cache_proxy_response(&cache_key, &to_cache, ttl)
                .await
            {
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
    matches!(
        name.to_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailers"
            | "transfer-encoding"
            | "upgrade"
    )
}
