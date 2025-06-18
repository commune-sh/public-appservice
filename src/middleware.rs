use axum::{
    body::Body,
    extract::{
        Path, 
        State, 
        OriginalUri, 
        MatchedPath
    },
    http::{
        Request, 
        StatusCode, 
        Uri, 
        header::AUTHORIZATION
    },
    response::IntoResponse,
    middleware::Next,
    Json,
    Extension
};

use ruma::{
    RoomId, 
    RoomAliasId,
};

use serde_json::{
    json, 
    Value
};

use std::sync::Arc;

use crate::AppState;
use crate::utils::room_id_valid;

use crate::error::AppserviceError;

pub fn extract_token(header: &str) -> Option<&str> {
    header.strip_prefix("Bearer ").map(|token| token.trim())
}

fn unauthorized_error() -> (StatusCode, Json<Value>) {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "errcode": "M_FORBIDDEN",
        }))
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


#[derive(Clone)]
pub struct Data {
    pub modified_path: Option<String>,
    pub room_id: Option<String>,
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
    };


    // This is a valid room_id, so move on
    if room_id_valid(&room_id, &server_name).is_ok() {
        req.extensions_mut().insert(data);
        return Ok(next.run(req).await);
    }


    let raw_alias = format!("#{}:{}", room_id, server_name);


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
                    format!("{}?{}", new_path, query).parse::<Uri>().unwrap_or_default()
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
        .ok_or(AppserviceError::AppserviceError("No room ID found".to_string()))?;

    let id = RoomId::parse(room_id)
        .map_err(|_| AppserviceError::AppserviceError("Invalid room ID".to_string()))?;

    let joined = state.appservice.has_joined_room(id)
        .await
        .map_err(|_| AppserviceError::AppserviceError("Failed to check room membership".to_string()))?;

    if !joined {
        return Err(AppserviceError::AppserviceError("User is not in room".to_string()));
    }

    Ok(next.run(req).await)
}

pub async fn is_public_room(
    Extension(data): Extension<Data>,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppserviceError> {

    let room_id = data
        .room_id
        .as_ref()
        .ok_or(AppserviceError::AppserviceError("No room ID found".to_string()))?;

    let id = RoomId::parse(room_id)
        .map_err(|_| AppserviceError::AppserviceError("Invalid room ID".to_string()))?;

    let joined = state.appservice.has_joined_room(id)
        .await
        .map_err(|_| AppserviceError::AppserviceError("Failed to check room membership".to_string()))?;

    if joined {
        return Ok((
            StatusCode::OK,
            Json(json!({
                "public": true,
            }))
        ))
    }

    Ok((
        StatusCode::OK,
        Json(json!({
            "public": false,
        }))
    ))
}
