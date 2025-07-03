use serde_json::json;
use thiserror::Error;

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

#[derive(Error, Debug)]
pub enum AppserviceError {
    #[error("{0}")]
    AppserviceError(String),
    #[error("Homeserver unreachable: {0}")]
    HomeserverError(String),
    #[error("Matrix API error: {0}")]
    MatrixError(String),
    #[error("Event not found: {0}")]
    EventNotFound(String),
    #[error("M_FORBIDDEN")]
    IncorrectHSToken,
}

impl IntoResponse for AppserviceError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppserviceError::AppserviceError(_) => (StatusCode::NOT_FOUND, self.to_string()),
            AppserviceError::HomeserverError(_) => (StatusCode::BAD_GATEWAY, self.to_string()),
            AppserviceError::MatrixError(_) => (StatusCode::NOT_FOUND, self.to_string()),
            AppserviceError::EventNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            AppserviceError::IncorrectHSToken => (StatusCode::UNAUTHORIZED, self.to_string()),
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}
