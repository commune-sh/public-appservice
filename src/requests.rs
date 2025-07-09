use axum::{
    Extension,
    body::Body,
    extract::{OriginalUri, State},
    http::{HeaderMap, Request, Response, StatusCode},
};

use std::time::Duration;

use std::sync::Arc;

use sha2::{Digest, Sha256};

use crate::AppState;
use crate::middleware::{Data, ProxyRequestType};

use crate::cache::CacheKey;

pub async fn matrix_proxy(
    Extension(data): Extension<Data>,
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
) -> Result<Response<Body>, StatusCode> {
    let method = req.method().clone();
    let headers = req.headers().clone();

    let path = if let Some(mod_path) = data.modified_path.as_ref() {
        mod_path.as_str()
    } else if let Some(original_uri) = req.extensions().get::<OriginalUri>() {
        original_uri.0.path()
    } else {
        req.uri().path()
    };

    let mut target_url = format!("{}{}", state.config.matrix.homeserver, path);

    if data.modified_path.is_none() {
        if let Some(query) = req.uri().query() {
            target_url.push('?');
            target_url.push_str(query);
        }
    }

    let cache_key = ("proxy_request", target_url.as_str()).cache_key();

    let skip_cache: bool = match data.proxy_request_type {
        ProxyRequestType::RoomState => !state.config.cache.room_state.enabled,
        ProxyRequestType::Messages => !state.config.cache.messages.enabled,
        ProxyRequestType::Media => true,
        ProxyRequestType::Other => false,
    };

    // skip if cache disabled by config for request type
    if !state.config.cache.requests.enabled || skip_cache {
        return proxy_request_no_cache(state, method, headers, target_url, req).await;
    }

    if let Ok(Some(cached_response)) = state.cache.get_cached_data::<Vec<u8>>(&cache_key).await {
        tracing::info!(
            "Returning cached proxy response for {} ({} bytes)",
            target_url,
            cached_response.len()
        );

        return Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(axum::body::Body::from(cached_response))
            .map_err(|e| {
                tracing::error!("Failed to build cached response: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            });
    }

    // cache missed
    let response_data = state
        .cache
        .cache_or_fetch(&cache_key, state.config.cache.requests.ttl, || async {
            tracing::info!("Cache miss for proxy request: {}", target_url);

            let body_bytes = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
                Ok(bytes) => bytes,
                Err(_) => {
                    return Err(redis::RedisError::from((
                        redis::ErrorKind::IoError,
                        "Failed to read request body",
                    )));
                }
            };

            let mut request_builder = state
                .proxy
                .request(method.clone(), &target_url)
                .timeout(Duration::from_secs(25))
                .bearer_auth(&state.config.appservice.access_token);

            let mut filtered_headers = HeaderMap::new();
            for (name, value) in headers.iter() {
                if !is_hop_by_hop_header(name.as_str()) && name != "authorization" {
                    filtered_headers.insert(name, value.clone());
                }
            }

            request_builder = request_builder.headers(filtered_headers);

            if !body_bytes.is_empty() {
                request_builder = request_builder.body(body_bytes);
            }

            let response = request_builder.send().await.map_err(|e| {
                tracing::error!("Proxy request failed for {}: {}", target_url, e);
                redis::RedisError::from((redis::ErrorKind::IoError, "Proxy request failed"))
            })?;

            let body = response.bytes().await.map_err(|e| {
                tracing::error!(
                    "Failed to read proxy response body for {}: {}",
                    target_url,
                    e
                );
                redis::RedisError::from((redis::ErrorKind::IoError, "Failed to read response body"))
            })?;

            let response_vec = body.to_vec();
            tracing::info!(
                "Fetched and cached proxy response for {} ({} bytes)",
                target_url,
                response_vec.len()
            );

            Ok(response_vec)
        })
        .await
        .map_err(|e| {
            tracing::error!("Failed to get proxy response for {}: {}", target_url, e);
            StatusCode::BAD_GATEWAY
        })?;

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(axum::body::Body::from(response_data))
        .map_err(|e| {
            tracing::error!("Failed to build response: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

async fn proxy_request_no_cache(
    state: Arc<AppState>,
    method: axum::http::Method,
    headers: HeaderMap,
    target_url: String,
    req: Request<Body>,
) -> Result<Response<Body>, StatusCode> {
    let body_bytes = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let mut request_builder = state
        .proxy
        .request(method, &target_url)
        .timeout(Duration::from_secs(25))
        .bearer_auth(&state.config.appservice.access_token);

    let mut filtered_headers = HeaderMap::new();
    for (name, value) in headers.iter() {
        if !is_hop_by_hop_header(name.as_str()) && name != "authorization" {
            filtered_headers.insert(name, value.clone());
        }
    }

    request_builder = request_builder.headers(filtered_headers);

    if !body_bytes.is_empty() {
        request_builder = request_builder.body(body_bytes);
    }

    let response = request_builder
        .send()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    let status = response.status();
    let response_headers = response.headers().clone();
    let body = response
        .bytes()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut axum_response = Response::builder().status(status);

    for (name, value) in response_headers.iter() {
        if !is_hop_by_hop_header(name.as_str()) {
            axum_response = axum_response.header(name, value);
        }
    }

    axum_response
        .body(axum::body::Body::from(body))
        .map_err(|e| {
            tracing::error!("Failed to build response: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

pub async fn matrix_proxy_search(
    Extension(data): Extension<Data>,
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
) -> Result<Response<Body>, StatusCode> {
    let method = req.method().clone();
    let headers = req.headers().clone();

    let path = if let Some(mod_path) = data.modified_path.as_ref() {
        mod_path.as_str()
    } else if let Some(original_uri) = req.extensions().get::<OriginalUri>() {
        original_uri.0.path()
    } else {
        req.uri().path()
    };

    let mut target_url = format!("{}{}", state.config.matrix.homeserver, path);

    if data.modified_path.is_none() {
        if let Some(query) = req.uri().query() {
            target_url.push('?');
            target_url.push_str(query);
        }
    }

    let body_bytes = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::error!("Failed to read request body for {}: {}", &target_url, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let cache_key = if state.config.cache.search.enabled {
        let mut hasher = Sha256::new();
        hasher.update(&body_bytes);
        let body_hash = format!("{:x}", hasher.finalize());
        format!("proxy_post_request:{target_url}:{body_hash}")
    } else {
        String::new()
    };

    if state.config.cache.search.enabled {
        if let Ok(cached_response) = state.cache.get_cached_proxy_response(&cache_key).await {
            tracing::info!("Returning cached search response for {}", target_url);

            if let Ok(response) = Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(axum::body::Body::from(cached_response))
                .map_err(|e| {
                    tracing::error!("Failed to build response: {}", e);
                })
            {
                return Ok(response);
            }
        }
    }

    let mut request_builder = state
        .proxy
        .request(method, &target_url)
        .timeout(Duration::from_secs(25))
        .bearer_auth(&state.config.appservice.access_token);

    let mut filtered_headers = HeaderMap::new();
    for (name, value) in headers.iter() {
        if !is_hop_by_hop_header(name.as_str()) && name != "authorization" {
            filtered_headers.insert(name, value.clone());
        }
    }

    request_builder = request_builder.headers(filtered_headers);

    if !body_bytes.is_empty() {
        request_builder = request_builder.body(body_bytes);
    }

    let response = request_builder.send().await.map_err(|e| {
        tracing::error!("Failed to build request for {}: {}", target_url, e);
        StatusCode::BAD_GATEWAY
    })?;

    let status = response.status();
    let headers = response.headers().clone();
    let body = response.bytes().await.map_err(|e| {
        tracing::error!("Failed to read response body for {}: {}", target_url, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let to_cache = body.to_vec();
    let ttl = state.config.cache.search.ttl;

    if state.config.cache.search.enabled {
        tokio::spawn(async move {
            if (state
                .cache
                .cache_proxy_response(&cache_key, &to_cache, ttl)
                .await)
                .is_ok()
            {
                tracing::info!("Cached proxied search response for {}", target_url);
            } else {
                tracing::warn!("Failed to cache search response for {}", target_url);
            }
        });
    }

    let mut axum_response = Response::builder().status(status);

    for (name, value) in headers.iter() {
        if !is_hop_by_hop_header(name.as_str()) {
            axum_response = axum_response.header(name, value);
        }
    }

    let response = axum_response
        .body(axum::body::Body::from(body))
        .map_err(|e| {
            tracing::error!("Failed to build response: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(response)
}

fn is_hop_by_hop_header(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailers"
            | "transfer-encoding"
            | "upgrade"
    )
}
