use axum::{
    middleware::{self},
    routing::{get, put, post},
    http::HeaderValue,
    extract::Request,
    Router,
    ServiceExt,
    response::IntoResponse,
    Json,
};

use std::sync::Arc;
use tracing::info;

use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tower_http::normalize_path::NormalizePathLayer;
use tower::Layer;

use serde_json::json;


use http::header::CONTENT_TYPE;

use anyhow;

use crate::config::Config;
use crate::rooms::{public_rooms, room_info, join_room, leave_room};
use crate::middleware::{
    authenticate_homeserver,
    is_public_room,
    validate_public_room,
    validate_room_id,
};

use crate::ping::ping;

use crate::api::{
    transactions,
    matrix_proxy,
    media_proxy
};

pub struct Server{
    state: Arc<AppState>,
}

pub use crate::AppState;

impl Server {

    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state
        }
    }

    pub fn setup_cors(&self, config: &Config) -> CorsLayer {

        let mut layer = CorsLayer::new()
            .allow_origin(Any)
            .allow_headers(vec![CONTENT_TYPE]);

        layer = match &config.server.allow_origin {
            Some(origins) if !origins.is_empty() && 
            !origins.contains(&"".to_string()) &&
            !origins.contains(&"*".to_string()) => {
                let origins = origins.iter().filter_map(|s| s.parse::<HeaderValue>().ok()).collect::<Vec<_>>();
                layer.allow_origin(origins)
            },
            _ => layer,
        };

        layer
    }

    pub async fn run(&self) -> Result<(), anyhow::Error> {
        let ping_state = self.state.clone();

        let addr = format!("0.0.0.0:{}", &self.state.config.server.port);

        let service_routes = Router::new()
            .route("/ping", post(ping))
            .route("/transactions/:txn_id", put(transactions))
            .route_layer(middleware::from_fn_with_state(self.state.clone(), authenticate_homeserver));

        let room_routes_inner = Router::new()
            .route("/state", get(matrix_proxy))
            .route("/messages", get(matrix_proxy))
            .route("/info", get(room_info))
            .route("/joined_members", get(matrix_proxy))
            .route("/aliases", get(matrix_proxy))
            .route("/event/*path", get(matrix_proxy))
            .route("/context/*path", get(matrix_proxy))
            .route("/timestamp_to_event", get(matrix_proxy));

        let room_routes = Router::new()
            .nest("/:room_id", room_routes_inner)
            .route_layer(middleware::from_fn_with_state(self.state.clone(), validate_public_room))
            .route_layer(middleware::from_fn_with_state(self.state.clone(), validate_room_id));

        let public_room = Router::new()
            .route("/:room_id", get(is_public_room))
            .route_layer(middleware::from_fn_with_state(self.state.clone(), validate_room_id));


        let more_room_routes = Router::new()
            .route("/hierarchy", get(matrix_proxy))
            .route("/threads", get(matrix_proxy))
            .route("/relations/*path", get(matrix_proxy))
            .route_layer(middleware::from_fn_with_state(self.state.clone(), validate_public_room))
            .route_layer(middleware::from_fn_with_state(self.state.clone(), validate_room_id));

        let public_rooms_route = Router::new()
            .route("/", get(public_rooms));
            //.route_layer(middleware::from_fn_with_state(self.state.clone(), public_rooms_cache));

        let media_routes = Router::new()
            .route("/thumbnail/*path", get(media_proxy))
            .route("/download/*path", get(media_proxy));

        let app = Router::new()
            .nest("/_matrix/app/v1", service_routes)
            .nest("/_matrix/client/v3/rooms", room_routes)
            .nest("/_matrix/client/v3/public", public_room)
            .nest("/_matrix/client/v1/rooms/:room_id", more_room_routes)
            .nest("/_matrix/client/v1/media", media_routes)
            .nest("/publicRooms", public_rooms_route)
            .route("/join_room/:room_id", put(join_room))
            .route("/leave_room/:room_id", put(leave_room))
            .route("/version", get(version))
            .route("/", get(index))
            .layer(self.setup_cors(&self.state.config))
            .layer(TraceLayer::new_for_http())
            .with_state(self.state.clone());

        let app = NormalizePathLayer::trim_trailing_slash()
            .layer(app);


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

pub async fn version(
) -> Result<impl IntoResponse, ()> {

    let version = env!("CARGO_PKG_VERSION");
    let hash = env!("GIT_COMMIT_HASH");

    Ok(Json(json!({
        "version": version,
        "commit": hash,
    })))
}


