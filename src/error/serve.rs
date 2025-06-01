use serde_json::json;
use thiserror::Error;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

#[derive(Error, Debug)]
pub enum Main {
    #[error("Appservice error: {0}")]
    Appservice(String),

    #[error("Homeserver unreachable: {0}")]
    Homeserver(String),

    #[error("Matrix API error: {0}")]
    Matrix(String),

    #[error("Event not found: {0}")]
    EventNotFound(String),

    #[error("M_FORBIDDEN")]
    IncorrectHSToken,
}

impl IntoResponse for Main {
    fn into_response(self) -> Response {
        let status = match self {
            Main::Appservice(_) | Main::Homeserver(_) | Main::Matrix(_) => StatusCode::BAD_GATEWAY,
            Main::EventNotFound(_) => StatusCode::NOT_FOUND,
            Main::IncorrectHSToken => StatusCode::UNAUTHORIZED,
        };

        let body = Json(json!({ "error": self.to_string() }));

        (status, body).into_response()
    }
}
