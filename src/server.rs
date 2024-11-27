use axum::{
    body::Body,
    extract::{Path, State, OriginalUri, MatchedPath},
    http::{Request, Response, StatusCode, Uri, HeaderValue, header::AUTHORIZATION},
    middleware::{self, Next},
    response::IntoResponse,
    routing::{get, post, put, any},
    Json,
    Router,
    Extension
};

use serde_json::{json, Value};

use std::sync::Arc;
use tracing::info;

use hyper_util::{client::legacy::connect::HttpConnector, rt::TokioExecutor};

type Client = hyper_util::client::legacy::Client<HttpConnector, Body>;

use ruma::{
    RoomId, OwnedRoomId, OwnedRoomAliasId, RoomAliasId,
    events::{
        room::member::{RoomMemberEvent, MembershipState},
    },
    serde::Raw,
};




use anyhow;


use crate::config::Config;

use crate::appservice::AppService;

use crate::rooms::public_rooms;

use crate::middleware::{
    Data,
    authenticate_homeserver,
    validate_public_room,
    validate_room_id,
};

#[warn(dead_code)]
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
            .route("/ping", get(ping))
            .route("/transactions/:txn_id", put(transactions))
            .route_layer(middleware::from_fn_with_state(state.clone(), authenticate_homeserver))
            .with_state(state.clone());

        let room_routes_inner = Router::new()
            .route("/joined_members", get(proxy_handler))
            .route("/aliases", get(proxy_handler))
            .route("/event/*path", get(proxy_handler))
            .route("/context/*path", get(proxy_handler))
            .route("/timestamp_to_event", get(proxy_handler))
            .with_state(state.clone());

        let room_routes = Router::new()
            .nest("/:room_id", room_routes_inner)
            .route_layer(middleware::from_fn_with_state(state.clone(), validate_public_room))
            .route_layer(middleware::from_fn_with_state(state.clone(), validate_room_id))
            .with_state(state.clone());

        let more_room_routes = Router::new()
            .route("/hierarchy", get(proxy_handler))
            .route("/threads", get(proxy_handler))
            .route("/relations/*path", get(proxy_handler))
            .route_layer(middleware::from_fn_with_state(state.clone(), validate_public_room))
            .route_layer(middleware::from_fn_with_state(state.clone(), validate_room_id))
            .with_state(state.clone());

        let app = Router::new()
            .nest("/_matrix/app/v1", service_routes)
            .nest("/_matrix/client/v3/rooms", room_routes)
            .nest("/_matrix/client/v1/rooms/:rood_id", more_room_routes)
            .fallback(any(proxy_handler))
            .route("/", get(index))
            .route("/publicRooms", get(public_rooms))
            .with_state(state);


        let addr = format!("localhost:{}", port);

        if let Ok(listener) = tokio::net::TcpListener::bind(addr.clone()).await {
            info!("Starting Commune public appservice...");
            axum::serve(listener, app).await?;
        } else {
            eprintln!("Failed to bind to address: {}", addr);
            std::process::exit(1);
        }

        Ok(())
    }
}

async fn transactions(
    Path(txn_id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {

    let events = match payload.get("events") {
        Some(Value::Array(events)) => events,
        Some(_) | None => {
            println!("Events is not an array");
            return Ok(Json(serde_json::json!({})))
        }
    };


    for event in events {
        println!("Event: {:#?}", event);

        if let Ok(event) =  serde_json::from_value::<RoomMemberEvent>(event.clone()) {
            let room_id = event.room_id().to_owned();

            let membership = event.membership().to_owned();

            let server_name = event.room_id().server_name();

            match server_name {
                Some(server_name) => {
                    if server_name.as_str() != state.config.matrix.server_name {
                        println!("Ignoring event for room on different server: {}", server_name);
                        continue;
                    }
                }
                None => {
                    println!("Ignoring event for room with no server name");
                    continue;
                }
            }


            // Ignore membership events for other users
            let invited_user = event.state_key().to_owned();
            if invited_user != state.appservice.user_id {
                info!("Ignoring event for user: {}", invited_user);
                continue;
            }

            match membership {
                MembershipState::Leave => {
                    println!("Leaving room: {}", room_id);
                }
                MembershipState::Ban => {
                    println!("Banning user from room: {}", room_id);
                    state.appservice.leave_room(room_id).await;
                }
                MembershipState::Invite => {
                    println!("Joining room: {}", room_id);
                    state.appservice.join_room(room_id).await;
                }
                _ => {}
            }


        }
    }


    Ok(Json(serde_json::json!({})))
}


async fn proxy_handler(
    Extension(data): Extension<Data>,
    Path(params): Path<Vec<(String, String)>>,
    State(state): State<Arc<AppState>>,
    mut req: Request<Body>,
) -> Result<Response<Body>, StatusCode> {

    /*
    let room_id = params[0].1.clone();
    println!("does room id exist here?: {}", room_id);

    if let Some(room_id) = data.room_id.as_ref() {
        println!("passed down room id is: {:#?}", room_id);
    }
*/

    //let path = req.uri().path();
    let mut path = if let Some(path) = req.extensions().get::<OriginalUri>() {
        path.0.path()
    } else {
        req.uri().path()
    };

    if let Some(mod_path) = data.modified_path.as_ref() {
        path = mod_path;
    }

    //println!("final Path is: {}", path);

    let path_query = req.uri().query().map(|q| format!("?{}", q)).unwrap_or_default();

    //println!("Path query is: {}", path_query);

    let homeserver = &state.config.matrix.homeserver;

    let uri = format!("{}{}{}", homeserver, path, path_query);

    //println!("Proxying request to: {}", uri);

    *req.uri_mut() = Uri::try_from(uri).unwrap();

    let access_token = &state.config.appservice.access_token;

    //println!("Access token: {}", access_token);

    let auth_value = HeaderValue::from_str(&format!("Bearer {}", access_token))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    req.headers_mut().insert(AUTHORIZATION, auth_value);


    Ok(state.client
        .request(req)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .into_response())
}

async fn index() -> &'static str {
    "Commune public appservice.\n"
}

async fn ping(
    State(state): State<Arc<AppState>>,
) -> &'static str {
    let homeserver = &state.config.matrix.homeserver;
    println!("Pinging Homeserver: {}", homeserver);
    "Ping\n"
}
