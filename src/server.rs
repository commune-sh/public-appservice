use axum::{
    body::Body,
    middleware::{self},
    routing::{get, put, post},
    http::HeaderValue,
    Router,
};

use std::sync::Arc;
use tracing::info;
use hyper_util::{client::legacy::connect::HttpConnector, rt::TokioExecutor};

use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use http::header::CONTENT_TYPE;

use anyhow;

use crate::config::Config;
use crate::appservice::AppService;
use crate::cache::Cache;
use crate::rooms::{public_rooms, room_info};
use crate::middleware::{
    authenticate_homeserver,
    validate_public_room,
    validate_room_id,
};

use crate::ping::{
    TransactionStore,
    ping,
};
use crate::api::{
    transactions,
    matrix_proxy,
};

type Client = hyper_util::client::legacy::Client<HttpConnector, Body>;

pub struct Server {
    config: Config,
    appservice: AppService,
    cache: Cache,
}

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub client: Client,
    pub appservice: AppService,
    pub transaction_store: TransactionStore,
    pub cache: redis::Client,
}

impl Server {
    pub fn new(
        config: Config, 
        appservice: AppService,
        cache: Cache,
    ) -> Self {
        Self { config, appservice, cache }
    }

    pub fn setup_cors(&self) -> CorsLayer {

        let mut layer = CorsLayer::new()
            .allow_origin(Any)
            .allow_headers(vec![CONTENT_TYPE]);

        layer = match &self.config.server.allow_origin {
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

    pub async fn run(&self, port: u16) -> Result<(), anyhow::Error> {

        let client: Client =
        hyper_util::client::legacy::Client::<(), ()>::builder(TokioExecutor::new())
            .build(HttpConnector::new());

        let transaction_store = TransactionStore::new();

        let state = Arc::new(AppState {
            config: self.config.clone(),
            client,
            appservice: self.appservice.clone(),
            transaction_store,
            cache: self.cache.client.clone(),
        });

        let ping_state = state.clone();

        let service_routes = Router::new()
            .route("/ping", post(ping))
            .route("/transactions/:txn_id", put(transactions))
            .route_layer(middleware::from_fn_with_state(state.clone(), authenticate_homeserver));

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
            .route_layer(middleware::from_fn_with_state(state.clone(), validate_public_room))
            .route_layer(middleware::from_fn_with_state(state.clone(), validate_room_id));

        let more_room_routes = Router::new()
            .route("/hierarchy", get(matrix_proxy))
            .route("/threads", get(matrix_proxy))
            .route("/relations/*path", get(matrix_proxy))
            .route_layer(middleware::from_fn_with_state(state.clone(), validate_public_room))
            .route_layer(middleware::from_fn_with_state(state.clone(), validate_room_id));

        let public_rooms_route = Router::new()
            .route("/", get(public_rooms));
            //.route_layer(middleware::from_fn_with_state(state.clone(), public_rooms_cache));

        let app = Router::new()
            .nest("/_matrix/app/v1", service_routes)
            .nest("/_matrix/client/v3/rooms", room_routes)
            .nest("/_matrix/client/v1/rooms/:rood_id", more_room_routes)
            .nest("/publicRooms", public_rooms_route)
            .route("/", get(index))
            .layer(self.setup_cors())
            .layer(TraceLayer::new_for_http())
            .with_state(state);


        let addr = format!("0.0.0.0:{}", port);

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
            axum::serve(listener, app).await?;
        } else {
            eprintln!("Failed to bind to address: {}", addr);
            std::process::exit(1);
        }

        Ok(())
    }
}

async fn index() -> &'static str {
    "Matrix public appservice.\n"
}
