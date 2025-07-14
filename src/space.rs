use axum::{
    Json,
    extract::{Path, State},
    response::IntoResponse,
};

use ruma::RoomAliasId;

use crate::error::AppserviceError;

use crate::AppState;
use serde_json::json;
use std::sync::Arc;

use crate::appservice::RoomSummary;

use crate::cache::CacheKey;

pub async fn spaces(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppserviceError> {
    let default_spaces = state.config.spaces.default.clone();

    if default_spaces.is_empty() {
        return Err(AppserviceError::AppserviceError(
            "No default spaces configured".to_string(),
        ));
    }

    if !state.config.spaces.cache {
        // if caching is disabled, fetch directly
        let public_spaces = state.appservice.get_public_spaces().await.map_err(|e| {
            tracing::error!("Failed to get public spaces: {}", e);
            AppserviceError::AppserviceError("Failed to get public spaces".to_string())
        })?;

        return match public_spaces {
            Some(spaces) => Ok(Json(json!(spaces))),
            None => Err(AppserviceError::AppserviceError(
                "No public spaces found".to_string(),
            )),
        };
    }

    if let Ok(Some(cached_spaces)) = state
        .cache
        .get_cached_data::<Vec<RoomSummary>>("public_spaces")
        .await
    {
        if !cached_spaces.is_empty() {
            tracing::info!(
                "Returning cached public spaces ({} spaces)",
                cached_spaces.len()
            );
            return Ok(Json(json!(cached_spaces)));
        }
    }

    // cache missed
    let spaces = state
        .cache
        .cache_or_fetch("public_spaces", state.config.spaces.ttl, || async {
            tracing::info!("Cache miss for public spaces, fetching from appservice");

            let public_spaces = state.appservice.get_public_spaces().await.map_err(|e| {
                tracing::error!("Failed to get public spaces: {}", e);
                redis::RedisError::from((redis::ErrorKind::IoError, "Failed to get public spaces"))
            })?;

            match public_spaces {
                Some(spaces) => {
                    tracing::info!("Fetched and cached {} public spaces", spaces.len());
                    Ok(spaces)
                }
                None => {
                    tracing::warn!("No public spaces found");
                    Err(redis::RedisError::from((
                        redis::ErrorKind::ResponseError,
                        "No public spaces found",
                    )))
                }
            }
        })
        .await
        .map_err(|e| {
            tracing::error!("Failed to get public spaces: {}", e);
            AppserviceError::AppserviceError("Failed to get public spaces".to_string())
        })?;

    Ok(Json(json!(spaces)))
}

pub async fn space(
    State(state): State<Arc<AppState>>,
    Path(space): Path<String>,
) -> Result<impl IntoResponse, AppserviceError> {
    let server_name = &state.config.matrix.server_name;
    let raw_alias = format!("#{space}:{server_name}");

    let alias = RoomAliasId::parse(&raw_alias).map_err(|e| {
        tracing::error!("Failed to parse room alias: {}", e);
        AppserviceError::AppserviceError("No Alias".to_string())
    })?;

    if !state.config.spaces.cache {
        let room_id = state
            .appservice
            .room_id_from_alias(alias)
            .await
            .map_err(|e| {
                tracing::error!("Failed to get room ID from alias: {}", e);
                AppserviceError::AppserviceError("Space does not exist.".to_string())
            })?;

        let summary = state
            .appservice
            .get_room_summary(room_id)
            .await
            .map_err(|e| {
                tracing::error!("Failed to get room summary: {}", e);
                AppserviceError::AppserviceError("Failed to get space summary".to_string())
            })?;

        return Ok(Json(json!(summary)));
    }

    let cache_key = ("space_summary", space.clone()).cache_key();
    if let Ok(Some(cached_summary)) = state.cache.get_cached_data::<RoomSummary>(&cache_key).await {
        tracing::info!("Returning cached space summary for {}", space);
        return Ok(Json(json!(cached_summary)));
    }

    // cache missed
    let summary = state
        .cache
        .cache_or_fetch(&cache_key, state.config.spaces.ttl, || async {
            tracing::info!("Cache miss for space {}, fetching summary", space);

            let room_id = state
                .appservice
                .room_id_from_alias(alias)
                .await
                .map_err(|e| {
                    tracing::error!("Failed to get room ID from alias: {}", e);
                    redis::RedisError::from((redis::ErrorKind::IoError, "Space does not exist"))
                })?;

            let summary = state
                .appservice
                .get_room_summary(room_id)
                .await
                .map_err(|e| {
                    tracing::error!("Failed to get room summary: {}", e);
                    redis::RedisError::from((
                        redis::ErrorKind::IoError,
                        "Failed to get space summary",
                    ))
                })?;

            tracing::info!("Fetched and cached space summary for {}", space);
            Ok(summary)
        })
        .await
        .map_err(|e| {
            tracing::error!("Failed to get space summary: {}", e);
            AppserviceError::AppserviceError("Failed to get space summary".to_string())
        })?;

    Ok(Json(json!(summary)))
}

pub async fn space_rooms(
    State(state): State<Arc<AppState>>,
    Path(space): Path<String>,
) -> Result<impl IntoResponse, AppserviceError> {
    let server_name = &state.config.matrix.server_name;
    let raw_alias = format!("#{space}:{server_name}");

    let alias = RoomAliasId::parse(&raw_alias).map_err(|e| {
        tracing::error!("Failed to parse room alias: {}", e);
        AppserviceError::AppserviceError("No Alias".to_string())
    })?;

    let room_id = state
        .appservice
        .room_id_from_alias(alias)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get room ID from alias: {}", e);
            AppserviceError::AppserviceError("Space does not exist.".to_string())
        })?;

    if state.config.spaces.cache {
        let cache_key = ("space_rooms", space.clone()).cache_key();

        // check cache first
        if let Ok(Some(cached_space_rooms)) = state
            .cache
            .get_cached_data::<Vec<RoomSummary>>(&cache_key)
            .await
        {
            tracing::info!(
                "Returning cached space rooms for {} ({} rooms)",
                space,
                cached_space_rooms.len()
            );
            return Ok(Json(json!(cached_space_rooms)));
        }

        let space_rooms = state
            .cache
            .cache_or_fetch(&cache_key, state.config.spaces.ttl, || async {
                tracing::info!("Cache miss for space hierarchy {}, fetching", space);

                let space_rooms = state
                    .appservice
                    .get_space_rooms(room_id.clone())
                    .await
                    .map_err(|e| {
                        tracing::error!("Failed to get space hierarchy: {}", e);
                        redis::RedisError::from((
                            redis::ErrorKind::IoError,
                            "Failed to get space hierarchy",
                        ))
                    })?;

                tracing::info!(
                    "Fetched and cached space hierarchy for {} ({} rooms)",
                    space,
                    space_rooms.len()
                );
                Ok(space_rooms)
            })
            .await
            .map_err(|e| {
                tracing::error!("Failed to get space rooms: {}", e);
                AppserviceError::AppserviceError("Failed to get space rooms".to_string())
            })?;

        return Ok(Json(json!(space_rooms)));
    }

    let space_rooms = state
        .appservice
        .get_space_rooms(room_id.clone())
        .await
        .map_err(|e| {
            tracing::error!("Failed to get space rooms: {}", e);
            AppserviceError::AppserviceError("Failed to get space rooms".to_string())
        })?;

    Ok(Json(json!(space_rooms)))
}
