use axum::{
    body::Body,
    extract::{OriginalUri, State},
    http::{header::AUTHORIZATION, HeaderValue, Request, Response, StatusCode, Uri},
    response::IntoResponse,
    Extension,
};

use ruma::events::macros::EventContent;

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{middleware::Data, Application};

pub mod event;
pub mod ping;

#[derive(Clone, Debug, Deserialize, Serialize, EventContent)]
#[ruma_event(type = "commune.public.room", kind = State, state_key_type = String)]
pub struct CommunePublicRoomEventContent {
    pub public: bool,
}

pub async fn matrix_proxy(
    Extension(data): Extension<Data>,
    State(state): State<Arc<Application>>,
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

    let path_query = req
        .uri()
        .query()
        .map(|q| format!("?{}", q))
        .unwrap_or_default();

    let homeserver = &state.config.matrix.homeserver;

    // add path query if path wasn't modified in middleware
    let uri = if data.modified_path.is_some() {
        format!("{}{}", homeserver, path)
    } else {
        format!("{}{}{}", homeserver, path, path_query)
    };

    *req.uri_mut() = Uri::try_from(uri).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let access_token = &state.config.appservice.access_token;

    let auth_value = HeaderValue::from_str(&format!("Bearer {}", access_token))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    req.headers_mut().insert(AUTHORIZATION, auth_value);

    let response = state
        .proxy
        .request(req)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .into_response();

    Ok(response)
}

pub async fn media_proxy(
    State(state): State<Arc<Application>>,
    mut req: Request<Body>,
) -> Result<Response<Body>, StatusCode> {
    let path = if let Some(path) = req.extensions().get::<OriginalUri>() {
        path.0.path()
    } else {
        req.uri().path()
    };

    let path_query = req
        .uri()
        .query()
        .map(|q| format!("?{}", q))
        .unwrap_or_default();

    let homeserver = &state.config.matrix.homeserver;

    // add path query if path wasn't modified in middleware
    let uri = format!("{}{}{}", homeserver, path, path_query);

    *req.uri_mut() = Uri::try_from(uri).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let access_token = &state.config.appservice.access_token;

    let auth_value = HeaderValue::from_str(&format!("Bearer {}", access_token))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    req.headers_mut().insert(AUTHORIZATION, auth_value);

    let response = state
        .proxy
        .request(req)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .into_response();

    Ok(response)
}
