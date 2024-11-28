use thiserror::Error;
use serde_json::json;

use axum::{
    Json,
    response::{IntoResponse, Response},
    http::StatusCode,
};


#[derive(Error, Debug)]
pub enum AppserviceError {
    #[error("Homeserver unreachable: {0}")]
    HomeserverError(String),
    #[error("Matrix API error: {0}")]
    MatrixError(String),
    #[error("Event not found: {0}")]
    EventNotFound(String),
}

impl IntoResponse for AppserviceError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppserviceError::HomeserverError(_) => (StatusCode::BAD_GATEWAY, self.to_string()),
            AppserviceError::MatrixError(_) => (StatusCode::BAD_GATEWAY, self.to_string()),
            AppserviceError::EventNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}

