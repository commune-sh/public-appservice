use axum::{
    Json, Router, ServiceExt,
    extract::{Request, State},
    http::HeaderValue,
    middleware::{self},
    response::IntoResponse,
    routing::{get, post, put},
};

use std::sync::Arc;
use tracing::info;

use tower::Layer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::normalize_path::NormalizePathLayer;
use tower_http::trace::TraceLayer;

use serde_json::json;

use http::header::CONTENT_TYPE;

use crate::error::AppserviceError;
use anyhow;

use crate::config::Config;
use crate::middleware::{
    add_data, authenticate_homeserver, is_admin, validate_public_room, validate_room_id,
};
use crate::rooms::{join_room, leave_room, public_rooms, room_info};

use crate::ping::ping;

use crate::api::{matrix_proxy, matrix_proxy_search, transactions};

use crate::space::{space, space_rooms, spaces};

pub struct Server {
    state: Arc<AppState>,
}

pub use crate::AppState;

impl Server {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    pub fn setup_cors(&self, config: &Config) -> CorsLayer {
        let mut layer = CorsLayer::new()
            .allow_origin(Any)
            .allow_headers(vec![CONTENT_TYPE]);

        layer = match &config.server.allow_origin {
            Some(origins)
                if !origins.is_empty()
                    && !origins.contains(&"".to_string())
                    && !origins.contains(&"*".to_string()) =>
            {
                let origins = origins
                    .iter()
                    .filter_map(|s| s.parse::<HeaderValue>().ok())
                    .collect::<Vec<_>>();
                layer.allow_origin(origins)
            }
            _ => layer,
        };

        layer
    }

    pub async fn run(&self) -> Result<(), anyhow::Error> {
        let ping_state = self.state.clone();

        let addr = format!("0.0.0.0:{}", &self.state.config.server.port);

        let service_routes = Router::new()
            .route("/_matrix/app/v1/ping", post(ping))
            .route("/_matrix/app/v1/transactions/{txn_id}", put(transactions))
            .route_layer(middleware::from_fn_with_state(
                self.state.clone(),
                authenticate_homeserver,
            ));

        let room_routes_inner = Router::new()
            .route("/state", get(matrix_proxy))
            .route("/state/{*path}", get(matrix_proxy))
            .route("/events", get(matrix_proxy))
            .route("/messages", get(matrix_proxy))
            .route("/info", get(room_info))
            .route("/joined_members", get(matrix_proxy))
            .route("/members", get(matrix_proxy))
            .route("/initialSync", get(matrix_proxy))
            .route("/aliases", get(matrix_proxy))
            .route("/event/{*path}", get(matrix_proxy))
            .route("/context/{*path}", get(matrix_proxy))
            .route("/timestamp_to_event", get(matrix_proxy));

        let room_routes = Router::new()
            .nest("/_matrix/client/v3/rooms/{room_id}", room_routes_inner)
            .route_layer(middleware::from_fn_with_state(
                self.state.clone(),
                validate_public_room,
            ))
            .route_layer(middleware::from_fn_with_state(
                self.state.clone(),
                validate_room_id,
            ));

        let more_room_routes = Router::new()
            .route(
                "/_matrix/client/v1/rooms/{room_id}/hierarchy",
                get(matrix_proxy),
            )
            .route(
                "/_matrix/client/v1/rooms/{room_id}/threads",
                get(matrix_proxy),
            )
            .route(
                "/_matrix/client/v1/rooms/{room_id}/relations/{*path}",
                get(matrix_proxy),
            )
            .route_layer(middleware::from_fn_with_state(
                self.state.clone(),
                validate_public_room,
            ))
            .route_layer(middleware::from_fn_with_state(
                self.state.clone(),
                validate_room_id,
            ));

        let public_rooms_route = Router::new().route("/publicRooms", get(public_rooms));

        let media_routes = Router::new()
            .route("/_matrix/client/v1/media/preview_url", get(matrix_proxy))
            .route(
                "/_matrix/client/v1/media/thumbnail/{*path}",
                get(matrix_proxy),
            )
            .route(
                "/_matrix/client/v1/media/download/{*path}",
                get(matrix_proxy),
            );

        let admin_routes = Router::new()
            .route("/admin/room/{room_id}/join", put(join_room))
            .route("/admin/room/{room_id}/leave", put(leave_room))
            .route_layer(middleware::from_fn_with_state(self.state.clone(), is_admin));

        let spaces_routes = Router::new()
            .route("/spaces/{space}/rooms", get(space_rooms))
            .route("/spaces/{space}", get(space))
            .route("/spaces", get(spaces));

        let search_route =
            Router::new().route("/_matrix/client/v3/search", post(matrix_proxy_search));

        let app = Router::new()
            .merge(service_routes)
            .merge(room_routes)
            .merge(more_room_routes)
            .merge(media_routes)
            .merge(public_rooms_route)
            .merge(admin_routes)
            .merge(spaces_routes);

        let app = if !self.state.config.search.disabled {
            app.merge(search_route)
        } else {
            app
        };

        let app = app
            .route("/version", get(version))
            .route("/identity", get(identity))
            .route("/health", get(health))
            .route("/", get(index))
            .layer(self.setup_cors(&self.state.config))
            .layer(middleware::from_fn_with_state(self.state.clone(), add_data))
            .layer(TraceLayer::new_for_http())
            .with_state(self.state.clone());

        let app = NormalizePathLayer::trim_trailing_slash().layer(app);

        tokio::spawn(async move {
            info!("Pinging homeserver...");
            let txn_id = ping_state.transaction_store.generate_transaction_id().await;
            let ping = ping_state.appservice.ping_homeserver(txn_id.clone()).await;
            match ping {
                Ok(_) => info!("Homeserver pinged successfully."),
                Err(e) => tracing::info!("Failed to ping homeserver: {}", e),
            }
        });

        if let Ok(listener) = tokio::net::TcpListener::bind(addr.clone()).await {
            axum::serve(listener, ServiceExt::<Request>::into_make_service(app)).await?;
        } else {
            tracing::info!("Failed to bind to address: {}", addr);
            return Err(anyhow::anyhow!("Failed to bind to address: {}", addr));
        }

        Ok(())
    }
}

async fn index() -> &'static str {
    "Commune public appservice.\n"
}

pub async fn version() -> Result<impl IntoResponse, ()> {
    let version = env!("CARGO_PKG_VERSION");
    let hash = env!("GIT_COMMIT_HASH");

    Ok(Json(json!({
        "version": version,
        "commit": hash,
    })))
}

pub async fn identity(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, ()> {
    let user = format!(
        "@{}:{}",
        state.config.appservice.sender_localpart, state.config.matrix.server_name
    );

    Ok(Json(json!({
        "user": user,
    })))
}

pub async fn health(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppserviceError> {
    state.appservice
        .health_check()
        .await
        .map_err(|e| {
            tracing::error!("Health check failed: {}", e);
            AppserviceError::HomeserverError("Health check failed. Could not reach homeserver.".to_string())
        })?;

    let user = format!(
        "@{}:{}",
        state.config.appservice.sender_localpart, state.config.matrix.server_name
    );

    let search_disabled = state.config.search.disabled;

    let features = json!({
        "search_disabled": search_disabled,
    });

    Ok(Json(json!({
        "status": "ok",
        "user_id": user,
        "features": features,
    })))
}
