use axum::{
    extract::{Request, State},
    http::HeaderValue,
    middleware::{self},
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router, ServiceExt,
};
use std::sync::Arc;
use tracing::info;

use tower::Layer;
use tower_http::{
    cors::{Any, CorsLayer},
    normalize_path::NormalizePathLayer,
    trace::TraceLayer,
};

use serde_json::json;

use http::header::CONTENT_TYPE;

use anyhow;

use crate::{
    config::Config,
    middleware::{
        authenticate_homeserver, is_admin, is_public_room, validate_public_room, validate_room_id,
    },
    rooms::{join_room, leave_room, public_rooms, room_info},
};

use crate::ping::ping;

use crate::api::{matrix_proxy, media_proxy, transactions};

mod ext;

pub struct Server {
    state: Arc<Application>,
}

pub use crate::Application;

impl Server {
    pub fn new(state: Arc<Application>) -> Self {
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
            .route("/messages", get(matrix_proxy))
            .route("/info", get(room_info))
            .route("/joined_members", get(matrix_proxy))
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

        let public_room = Router::new()
            .route("/_matrix/client/v3/public/{room_id}", get(is_public_room))
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
        //.route_layer(middleware::from_fn_with_state(self.state.clone(), public_rooms_cache));

        let media_routes = Router::new()
            .route(
                "/_matrix/client/v1/media/thumbnail/{*path}",
                get(media_proxy),
            )
            .route(
                "/_matrix/client/v1/media/download/{*path}",
                get(media_proxy),
            );

        let admin_routes = Router::new()
            .route("/admin/room/{room_id}/join", put(join_room))
            .route("/admin/room/{room_id}/leave", put(leave_room))
            .route_layer(middleware::from_fn_with_state(self.state.clone(), is_admin));

        let app = Router::new()
            .merge(service_routes)
            .merge(room_routes)
            .merge(public_room)
            .merge(more_room_routes)
            .merge(media_routes)
            .merge(public_rooms_route)
            .merge(admin_routes)
            .route("/version", get(version))
            .route("/identity", get(identity))
            .route("/", get(index))
            .layer(self.setup_cors(&self.state.config))
            .layer(TraceLayer::new_for_http())
            .with_state(self.state.clone());

        let app = NormalizePathLayer::trim_trailing_slash().layer(app);

        tokio::spawn(async move {
            info!("Pinging homeserver...");
            let txn_id = ping_state.transaction_store.generate_transaction_id().await;
            let ping = ping_state.appservice.ping_homeserver(txn_id.clone()).await;
            match ping {
                Ok(_) => info!("Homeserver pinged successfully."),
                Err(e) => eprintln!("Failed to ping homeserver: {}", e),
            }
        });

        if let Ok(listener) = tokio::net::TcpListener::bind(addr.clone()).await {
            axum::serve(listener, ServiceExt::<Request>::into_make_service(app)).await?;
        } else {
            eprintln!("Failed to bind to address: {}", addr);
            std::process::exit(1);
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

pub async fn identity(State(state): State<Arc<Application>>) -> Result<impl IntoResponse, ()> {
    let user = format!(
        "@{}:{}",
        state.config.appservice.sender_localpart, state.config.matrix.server_name
    );

    Ok(Json(json!({
        "user": user,
    })))
}
