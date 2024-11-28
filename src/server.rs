use axum::{
    body::Body,
    middleware::{self},
    routing::{get, put, post},
    Router,
};

use std::sync::Arc;
use hyper_util::{client::legacy::connect::HttpConnector, rt::TokioExecutor};

use anyhow;

use crate::config::Config;
use crate::appservice::AppService;
use crate::rooms::{public_rooms, room_info};
use crate::middleware::{
    authenticate_homeserver,
    validate_public_room,
    validate_room_id,
};

use crate::api::{
    ping,
    transactions,
    matrix_proxy,
};

type Client = hyper_util::client::legacy::Client<HttpConnector, Body>;

pub struct Server {
    config: Config,
    appservice: AppService,
}

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub client: Client,
    pub appservice: AppService,
}

impl Server {
    pub fn new(config: Config, appservice: AppService) -> Self {
        Self { config, appservice }
    }

    pub async fn run(&self, port: u16) -> Result<(), anyhow::Error> {

        let client: Client =
        hyper_util::client::legacy::Client::<(), ()>::builder(TokioExecutor::new())
            .build(HttpConnector::new());

        let state = Arc::new(AppState {
            config: self.config.clone(),
            client,
            appservice: self.appservice.clone(),
        });

        let service_routes = Router::new()
            //.layer(Extension(state.clone()))
            .route("/ping", post(ping))
            .route("/transactions/:txn_id", put(transactions))
            .route_layer(middleware::from_fn_with_state(state.clone(), authenticate_homeserver))
            .with_state(state.clone());

        let room_routes_inner = Router::new()
            .route("/state", get(matrix_proxy))
            .route("/messages", get(matrix_proxy))
            .route("/info", get(room_info))
            .route("/joined_members", get(matrix_proxy))
            .route("/aliases", get(matrix_proxy))
            .route("/event/*path", get(matrix_proxy))
            .route("/context/*path", get(matrix_proxy))
            .route("/timestamp_to_event", get(matrix_proxy))
            .with_state(state.clone());

        let room_routes = Router::new()
            .nest("/:room_id", room_routes_inner)
            .route_layer(middleware::from_fn_with_state(state.clone(), validate_public_room))
            .route_layer(middleware::from_fn_with_state(state.clone(), validate_room_id))
            .with_state(state.clone());

        let more_room_routes = Router::new()
            .route("/hierarchy", get(matrix_proxy))
            .route("/threads", get(matrix_proxy))
            .route("/relations/*path", get(matrix_proxy))
            .route_layer(middleware::from_fn_with_state(state.clone(), validate_public_room))
            .route_layer(middleware::from_fn_with_state(state.clone(), validate_room_id))
            .with_state(state.clone());

        let app = Router::new()
            .nest("/_matrix/app/v1", service_routes)
            .nest("/_matrix/client/v3/rooms", room_routes)
            .nest("/_matrix/client/v1/rooms/:rood_id", more_room_routes)
            .route("/publicRooms", get(public_rooms))
            .route("/", get(index))
            .with_state(state);


        let addr = format!("localhost:{}", port);

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
