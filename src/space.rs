use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};

use ruma::RoomAliasId;

use crate::error::AppserviceError;

use std::sync::Arc;
use serde_json::json;
use crate::AppState;

pub async fn space_state(
    State(state): State<Arc<AppState>>,
    Path(space): Path<String>,
) -> Result<impl IntoResponse, AppserviceError> {


    let server_name = state.config.matrix.server_name.clone();

    let raw_alias = format!("#{}:{}", space, server_name);

    let alias = RoomAliasId::parse(&raw_alias)
        .map_err(|_| AppserviceError::AppserviceError("No Alias".to_string()))?;

    let room_id = state.appservice.room_id_from_alias(alias).await
        .map_err(|_| AppserviceError::AppserviceError("Not a valid space".to_string()))?;

    let hierarchy = state.appservice.get_room_hierarchy(room_id.clone())
        .await
        .map_err(|_| AppserviceError::AppserviceError("Failed to get room hierarchy".to_string()))?;

    Ok(Json(json!({
        "space": hierarchy,
    })))
}
