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

pub async fn spaces(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppserviceError> {
    let default_spaces = state.config.spaces.default.clone();

    if default_spaces.is_empty() {
        return Err(AppserviceError::AppserviceError(
            "No default spaces configured".to_string(),
        ));
    }

    // Return from cache if enabled and available
    if state.config.spaces.cache {
        if let Some(cached_spaces) = state.cache.get_cached_public_spaces().await.ok() {
            if !cached_spaces.is_empty() {
                tracing::info!("Returning cached public spaces");
                return Ok(Json(json!(cached_spaces)));
            }
        }
    }

    let public_spaces =
        state.appservice.get_public_spaces().await.map_err(|_| {
            AppserviceError::AppserviceError("Failed to get public spaces".to_string())
        })?;

    match public_spaces {
        Some(spaces) => {
            let to_cache = spaces.clone();

            // Cache public spaces if enabled
            if state.config.spaces.cache {
                tokio::spawn(async move {
                    if let Ok(_) = state
                        .cache
                        .cache_public_spaces(&to_cache, state.config.spaces.ttl)
                        .await
                    {
                        tracing::info!("Cached public spaces successfully");
                    } else {
                        tracing::warn!("Failed to cache public spaces");
                    }
                });
            }

            return Ok(Json(json!(spaces)));
        }
        None => {
            return Err(AppserviceError::AppserviceError(
                "No public spaces found".to_string(),
            ));
        }
    }
}

pub async fn space(
    State(state): State<Arc<AppState>>,
    Path(space): Path<String>,
) -> Result<impl IntoResponse, AppserviceError> {
    let server_name = state.config.matrix.server_name.clone();

    let raw_alias = format!("#{}:{}", space, server_name);

    let alias = RoomAliasId::parse(&raw_alias)
        .map_err(|_| AppserviceError::AppserviceError("No Alias".to_string()))?;

    let room_id = state
        .appservice
        .room_id_from_alias(alias)
        .await
        .map_err(|_| AppserviceError::AppserviceError("Space does not exist.".to_string()))?;
    //
    // Return from cache if enabled and available
    if state.config.spaces.cache {
        let key = format!("space_summary:{}", space);
        if let Some(summary) = state.cache.get_cached_data::<RoomSummary>(&key).await.ok() {
            tracing::info!("Returning cached space summary for {}", space);
            return Ok(Json(json!(summary)));
        }
    }

    let summary = state
        .appservice
        .get_room_summary(room_id.clone())
        .await
        .map_err(|_| AppserviceError::AppserviceError("Failed to get space summary".to_string()))?;

    if state.config.spaces.cache {
        let summary = summary.clone();
        tokio::spawn(async move {
            let key = format!("space_summary:{}", space);
            if let Ok(_) = state
                .cache
                .cache_data(&key, &summary, state.config.spaces.ttl)
                .await
            {
                tracing::info!("Cached space summary for {}", space);
            } else {
                tracing::warn!("Failed to cache space summary for {}", space);
            }
        });
    }

    Ok(Json(json!(summary)))
}

pub async fn space_rooms(
    State(state): State<Arc<AppState>>,
    Path(space): Path<String>,
) -> Result<impl IntoResponse, AppserviceError> {
    let server_name = state.config.matrix.server_name.clone();

    let raw_alias = format!("#{}:{}", space, server_name);

    let alias = RoomAliasId::parse(&raw_alias)
        .map_err(|_| AppserviceError::AppserviceError("No Alias".to_string()))?;

    let room_id = state
        .appservice
        .room_id_from_alias(alias)
        .await
        .map_err(|_| AppserviceError::AppserviceError("Space does not exist.".to_string()))?;

    let hierarchy = state
        .appservice
        .get_room_hierarchy(room_id.clone())
        .await
        .map_err(|_| {
            AppserviceError::AppserviceError("Failed to get space hierarchy".to_string())
        })?;

    Ok(Json(json!(hierarchy)))
}
