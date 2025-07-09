use axum::{
    Extension, Json,
    body::Body,
    extract::{MatchedPath, OriginalUri, Path, State},
    http::{Request, StatusCode, Uri, header::AUTHORIZATION},
    middleware::Next,
    response::IntoResponse,
};

use ruma::{OwnedRoomId, RoomAliasId, RoomId};

use serde_json::{Value, json};

use std::sync::Arc;

use crate::AppState;
use crate::utils::{room_alias_like, room_id_valid};

use crate::error::AppserviceError;

use crate::cache::CacheKey;

pub fn extract_token(header: &str) -> Option<&str> {
    header.strip_prefix("Bearer ").map(|token| token.trim())
}

fn unauthorized_error() -> (StatusCode, Json<Value>) {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "errcode": "M_FORBIDDEN",
        })),
    )
}

pub async fn authenticate_homeserver(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Result<impl IntoResponse, (StatusCode, Json<Value>)> {
    let token = req
        .headers()
        .get(AUTHORIZATION)
        .ok_or(unauthorized_error())?
        .to_str()
        .map_err(|_| unauthorized_error())?;

    let token = extract_token(token).ok_or(unauthorized_error())?;

    if token != state.config.appservice.hs_access_token {
        return Err(unauthorized_error());
    }

    Ok(next.run(req).await)
}

pub async fn is_admin(
    //State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Result<impl IntoResponse, (StatusCode, Json<Value>)> {
    let token = req
        .headers()
        .get(AUTHORIZATION)
        .ok_or(unauthorized_error())?
        .to_str()
        .map_err(|_| unauthorized_error())?;

    let token = extract_token(token).ok_or(unauthorized_error())?;

    if token != "test" {
        return Err(unauthorized_error());
    }

    Ok(next.run(req).await)
}

#[derive(Clone, Debug)]
pub enum ProxyRequestType {
    RoomState,
    Messages,
    Media,
    Other,
}

#[derive(Clone, Debug)]
pub struct Data {
    pub modified_path: Option<String>,
    pub room_id: Option<String>,
    pub is_media_request: bool,
    pub proxy_request_type: ProxyRequestType,
}

pub fn parse_request_type(req: &Request<Body>) -> ProxyRequestType {
    match req.uri().path() {
        path if path.ends_with("/state") => ProxyRequestType::RoomState,
        path if path.ends_with("/messages") => ProxyRequestType::Messages,
        path if path.starts_with("/_matrix/client/v1/media/") => ProxyRequestType::Media,
        _ => ProxyRequestType::Other,
    }
}

pub async fn add_data(
    mut req: Request<Body>,
    next: Next,
) -> Result<impl IntoResponse, (StatusCode, Json<Value>)> {
    let data = Data {
        modified_path: None,
        room_id: None,
        is_media_request: req.uri().path().starts_with("/_matrix/client/v1/media/"),
        proxy_request_type: parse_request_type(&req),
    };

    req.extensions_mut().insert(data);

    Ok(next.run(req).await)
}

pub async fn validate_room_id(
    Path(params): Path<Vec<(String, String)>>,
    State(state): State<Arc<AppState>>,
    mut req: Request<Body>,
    next: Next,
) -> Result<impl IntoResponse, (StatusCode, Json<Value>)> {
    let room_id = params[0].1.clone();

    let server_name = state.config.matrix.server_name.clone();

    let mut data = Data {
        modified_path: None,
        room_id: Some(room_id.clone()),
        is_media_request: req.uri().path().starts_with("/_matrix/media/v1/download/"),
        proxy_request_type: parse_request_type(&req),
    };

    // This is a valid room_id, so move on
    if room_id_valid(&room_id, &server_name).is_ok() {
        req.extensions_mut().insert(data);
        return Ok(next.run(req).await);
    }

    // If the alias is partial like room:server.com without the leading #, we assume it's a room alias
    let raw_alias = room_alias_like(&room_id)
        .then_some(format!("#{room_id}"))
        .unwrap_or_else(|| format!("#{room_id}:{server_name}"));

    if let Ok(alias) = RoomAliasId::parse(&raw_alias) {
        let id = state.appservice.room_id_from_alias(alias).await;
        match id {
            Ok(id) => {
                data.room_id = Some(id.to_string());
            }
            Err(_) => {
                tracing::info!("Failed to get room ID from alias: {}", raw_alias);
            }
        }
    }

    if let Some(path) = req.extensions().get::<MatchedPath>() {
        let pattern = path.as_str();

        // Split into segments, skipping the empty first segment
        let pattern_segments: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();

        let fullpath = if let Some(path) = req.extensions().get::<OriginalUri>() {
            path.0.path()
        } else {
            req.uri().path()
        };

        let path_segments: Vec<&str> = fullpath.split('/').filter(|s| !s.is_empty()).collect();

        if let Some(segment_index) = pattern_segments.iter().position(|&s| s == "{room_id}") {
            let mut new_segments = path_segments.clone();
            if segment_index < new_segments.len() {
                new_segments[segment_index] = data.room_id.as_ref().unwrap_or(&room_id);

                // Rebuild the path with leading slash
                let new_path = format!("/{}", new_segments.join("/"));

                // Preserve query string if it exists
                let new_uri = if let Some(query) = req.uri().query() {
                    format!("{new_path}?{query}")
                        .parse::<Uri>()
                        .unwrap_or_default()
                } else {
                    new_path.parse::<Uri>().unwrap_or_default()
                };

                data.modified_path = Some(new_uri.to_string());
            }
        }
    }

    req.extensions_mut().insert(data);

    Ok(next.run(req).await)
}

pub async fn validate_public_room(
    Extension(data): Extension<Data>,
    //Path(params): Path<Vec<(String, String)>>,
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Result<impl IntoResponse, AppserviceError> {
    let room_id = data
        .room_id
        .as_ref()
        .ok_or(AppserviceError::AppserviceError(
            "No room ID found".to_string(),
        ))?;

    let parsed_room_id = RoomId::parse(room_id)
        .map_err(|_| AppserviceError::AppserviceError("Invalid room ID".to_string()))?;

    let is_joined = check_room_membership(&state, room_id, parsed_room_id).await?;

    if !is_joined {
        return Err(AppserviceError::AppserviceError(
            "Not a public room".to_string(),
        ));
    }

    Ok(next.run(req).await)
}

async fn check_room_membership(
    state: &AppState,
    room_id: &str,
    parsed_room_id: OwnedRoomId,
) -> Result<bool, AppserviceError> {
    if !state.config.cache.joined_rooms.enabled {
        return check_membership_direct(state, room_id, parsed_room_id).await;
    }

    let cache_key = ("appservice:joined", room_id).cache_key();

    match state.cache.get_cached_data::<bool>(&cache_key).await {
        Ok(Some(cached_result)) => {
            tracing::info!("Using cached joined status for room: {}", room_id);
            Ok(cached_result)
        }
        Ok(None) | Err(_) => {
            let joined = check_membership_direct(state, room_id, parsed_room_id).await?;

            if joined {
                cache_membership_result(state, &cache_key, room_id).await;
            }

            Ok(joined)
        }
    }
}

async fn check_membership_direct(
    state: &AppState,
    room_id: &str,
    parsed_room_id: OwnedRoomId,
) -> Result<bool, AppserviceError> {
    state
        .appservice
        .has_joined_room(parsed_room_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to check joined status for room {}: {}", room_id, e);
            AppserviceError::AppserviceError("Failed to check joined status".to_string())
        })
}

async fn cache_membership_result(state: &AppState, cache_key: &str, room_id: &str) {
    match state.cache.cache_data(cache_key, &true, 300).await {
        Ok(_) => tracing::info!("Cached joined status for room: {}", room_id),
        Err(_) => tracing::warn!("Failed to cache joined status for room: {}", room_id),
    }
}
