use axum::{
    body::Body,
    extract::{Path, State, OriginalUri},
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
    events::{room::member::RoomMemberEvent},
    serde::Raw,
};




use anyhow;


use crate::config::Config;

use crate::appservice::AppService;

#[warn(dead_code)]
pub struct Server {
    config: Config,
    appservice: AppService,
}


#[derive(Clone)]
struct AppState {
    config: Config,
    client: Client,
    appservice: AppService,
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

        let login_routes = Router::new()
            .route(
                "/",
                get(login)
                    .route_layer(middleware::from_fn(login_get_middleware))
            )
            .route(
                "/",
                post(proxy_handler)
                    .route_layer(middleware::from_fn(login_post_middleware))
            )
            .with_state(state.clone());

        // Register endpoint with method-specific middleware
        let register_routes = Router::new()
            .route(
                "/",
                get(proxy_handler)
                    .route_layer(middleware::from_fn(register_get_middleware))
            )
            .route(
                "/",
                post(proxy_handler)
                    .route_layer(middleware::from_fn(register_post_middleware))
            )
            .with_state(state.clone());


        let service_routes = Router::new()
            //.layer(Extension(state.clone()))
            .route("/ping", get(ping))
            .route("/transactions/:txn_id", put(transactions))
            .route_layer(middleware::from_fn_with_state(state.clone(), authenticate_homeserver))
            .with_state(state.clone());

        let app = Router::new()
            .nest("/_matrix/app/v1", service_routes)
            .nest("/_matrix/client/v3/login", login_routes)
            .nest("/_matrix/client/v3/register", register_routes)
            .fallback(any(proxy_handler))
            .route("/", get(index))
            .layer(middleware::from_fn(request_middleware))
            .layer(middleware::from_fn(response_middleware))
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
    println!("Transaction ID is : {}", txn_id);


    let events = match payload.get("events") {
        Some(Value::Array(events)) => events,
        Some(_) | None => {
            println!("Events is not an array");
            return Ok(Json(serde_json::json!({})))
        }
    };


    // iterate over events
    for event in events {
        println!("Event: {:#?}", event);

        if let Ok(event) =  serde_json::from_value::<RoomMemberEvent>(event.clone()) {
            // Handle the deserialized event
            //
            let room_id = event.room_id().to_owned();

            state.appservice.join_room(room_id).await;
        }
    }


    Ok(Json(serde_json::json!({})))
}

async fn login_get_middleware(
    req: Request<Body>,
    next: Next,
) -> Result<Response<Body>, StatusCode> {
    info!("Processing login GET request");
    // Add login GET-specific validation, transformation, etc.
    Ok(next.run(req).await)
}

async fn login_post_middleware(
    req: Request<Body>,
    next: Next,
) -> Result<Response<Body>, StatusCode> {
    info!("Processing login POST request");
    // Add login POST-specific validation, transformation, etc.
    // For example: validate login credentials format
    Ok(next.run(req).await)
}

async fn register_get_middleware(
    req: Request<Body>,
    next: Next,
) -> Result<Response<axum::body::Body>, StatusCode> {
    info!("Processing register GET request");
    // Add register GET-specific validation, transformation, etc.
    Ok(next.run(req).await)
}

async fn register_post_middleware(
    req: Request<Body>,
    next: Next,
) -> Result<Response<Body>, StatusCode> {
    info!("Processing register POST request");
    // Add register POST-specific validation, transformation, etc.
    // For example: validate registration payload format
    Ok(next.run(req).await)
}

async fn proxy_handler(
    State(state): State<Arc<AppState>>,
    mut req: Request<Body>,
) -> Result<Response<Body>, StatusCode> {



    //let path = req.uri().path();
    let path = if let Some(path) = req.extensions().get::<OriginalUri>() {
        // This will include `/api`
        path.0.path()
    } else {
        // The `OriginalUri` extension will always be present if using
        // `Router` unless another extractor or middleware has removed it
        req.uri().path()
    };

    println!("Path is: {}", path);

    let path_query = req.uri().query().map(|q| format!("?{}", q)).unwrap_or_default();

    println!("Path query is: {}", path_query);

    let homeserver = &state.config.matrix.homeserver;

    let uri = format!("{}{}{}", homeserver, path, path_query);

    println!("Proxying request to: {}", uri);

    *req.uri_mut() = Uri::try_from(uri).unwrap();

    let access_token = &state.config.appservice.access_token;

    println!("Access token: {}", access_token);

    let auth_value = HeaderValue::from_str(&format!("Bearer {}", access_token))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    req.headers_mut().insert(AUTHORIZATION, auth_value);


    Ok(state.client
        .request(req)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .into_response())
}

pub fn extract_token(header: &str) -> Option<&str> {
    if header.starts_with("Bearer ") {
        Some(header.trim_start_matches("Bearer ").trim())
    } else {
        None
    }
}

async fn authenticate_homeserver(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Result<impl IntoResponse, (StatusCode, Json<Value>)> {

    if let Some(auth_header) = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok()) {
        if let Some(token) = extract_token(auth_header) {
            if token == &state.config.appservice.hs_access_token {
                return Ok(next.run(req).await)
            }
        }
    };

    Err((
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "errcode": "BAD_ACCESS_TOKEN",
            "error": "access token invalid"
        }))
    ))
}


async fn request_middleware(
    req: Request<Body>,
    next: Next,
) -> Result<Response<Body>, StatusCode> {
    info!(
        "Incoming request: {:#?}",
        req.uri().path()
    );
    Ok(next.run(req).await)
}

async fn response_middleware(
    req: Request<Body>,
    next: Next,
) -> Result<Response<Body>, StatusCode> {
    let response = next.run(req).await;
    info!("Response status: {}", response.status());
    Ok(response)
}

async fn index() -> &'static str {
    "Commune public appservice.\n"
}

async fn login() -> &'static str {
    "Login\n"
}

async fn ping(
    State(state): State<Arc<AppState>>,
) -> &'static str {
    let homeserver = &state.config.matrix.homeserver;
    println!("Pinging Homeserver: {}", homeserver);
    "Ping\n"
}
