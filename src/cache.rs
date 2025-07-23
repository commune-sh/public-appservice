use crate::config::Config;
use redis::{AsyncCommands, RedisError};
use serde::{Deserialize, Serialize};

use crate::appservice::RoomSummary;
use crate::rooms::PublicRoom;

pub trait Cacheable: Serialize + for<'a> Deserialize<'a> + Send + Sync {}

impl<T> Cacheable for T where T: Serialize + for<'a> Deserialize<'a> + Send + Sync {}

pub trait CacheKey {
    fn cache_key(&self) -> String;
}

impl CacheKey for String {
    fn cache_key(&self) -> String {
        self.clone()
    }
}

impl CacheKey for &str {
    fn cache_key(&self) -> String {
        self.to_string()
    }
}

impl CacheKey for (&str, &str) {
    fn cache_key(&self) -> String {
        format!("{}:{}", self.0, self.1)
    }
}

impl CacheKey for (&str, String) {
    fn cache_key(&self) -> String {
        format!("{}:{}", self.0, self.1)
    }
}

#[derive(Debug, Clone)]
pub struct Cache {
    pub client: redis::Client,
}

impl Cache {
    pub async fn new(config: &Config) -> Result<Self, anyhow::Error> {
        let url = format!("redis://{}", config.redis.url);
        let client = redis::Client::open(url)?;

        Ok(Self { client })
    }

    pub async fn cache_data<T>(&self, key: &str, data: &T, ttl: u64) -> Result<(), RedisError>
    where
        T: Cacheable,
    {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;

        let serialized = serde_json::to_string(data).map_err(|e| {
            RedisError::from((
                redis::ErrorKind::IoError,
                "Serialization error",
                e.to_string(),
            ))
        })?;

        let _: () = conn.set_ex(key, serialized, ttl).await?;
        Ok(())
    }

    pub async fn get_cached_data<T>(&self, key: &str) -> Result<Option<T>, RedisError>
    where
        T: Cacheable,
    {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;

        let exists: bool = conn.exists(key).await?;
        if !exists {
            return Ok(None);
        }

        let data: String = conn.get(key).await?;
        let value = serde_json::from_str(&data).map_err(|e| {
            RedisError::from((
                redis::ErrorKind::IoError,
                "Deserialization error",
                e.to_string(),
            ))
        })?;
        Ok(Some(value))
    }

    pub async fn cache_or_fetch<T, F, Fut>(
        &self,
        key: &str,
        ttl: u64,
        fetch_fn: F,
    ) -> Result<T, RedisError>
    where
        T: Cacheable,
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, RedisError>>,
    {
        if let Some(cached) = self.get_cached_data::<T>(key).await? {
            return Ok(cached);
        }

        let data = fetch_fn().await?;

        let _ = self.cache_data(key, &data, ttl).await;

        Ok(data)
    }

    pub async fn cache_with_ttl_threshold<T>(
        &self,
        key: &str,
        data: T,
        new_ttl: u64,
        ttl_threshold: u64,
    ) -> Result<(), RedisError>
    where
        T: Cacheable,
    {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;

        let ttl_remaining: i64 = conn.ttl(key).await?;

        tracing::info!("TTL remaining for key '{}': {}", key, ttl_remaining);

        let should_cache = match ttl_remaining {
            -2 => true,
            remaining if remaining < ttl_threshold as i64 => true,
            _ => false,
        };

        tracing::info!(
            "Should cache: {} (TTL remaining: {}, Threshold: {})",
            should_cache,
            ttl_remaining,
            ttl_threshold
        );

        if !should_cache {
            return Err(RedisError::from((
                redis::ErrorKind::ResponseError,
                "TTL remaining is greater than threshold, not caching",
            )));
        }

        self.cache_data(key, &data, new_ttl).await?;

        Ok(())
    }

    pub async fn cache_with_key<K, T>(&self, key: K, data: &T, ttl: u64) -> Result<(), RedisError>
    where
        K: CacheKey,
        T: Cacheable,
    {
        self.cache_data(&key.cache_key(), data, ttl).await
    }

    pub async fn get_with_key<K, T>(&self, key: K) -> Result<Option<T>, RedisError>
    where
        K: CacheKey,
        T: Cacheable,
    {
        self.get_cached_data(&key.cache_key()).await
    }

    pub async fn delete_cached_data(&self, key: &str) -> Result<(), RedisError> {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;
        let _: () = conn.del(key).await?;
        Ok(())
    }

    pub async fn cache_rooms(&self, rooms: &Vec<PublicRoom>, ttl: u64) -> Result<(), RedisError> {
        self.cache_data("public_rooms", rooms, ttl).await
    }

    pub async fn get_cached_rooms(&self) -> Result<Vec<PublicRoom>, RedisError> {
        self.get_cached_data("public_rooms")
            .await?
            .ok_or_else(|| RedisError::from((redis::ErrorKind::ResponseError, "Key not found")))
    }

    pub async fn get_cached_room_state(
        &self,
        room_id: &str,
    ) -> Result<Vec<PublicRoom>, RedisError> {
        let key = format!("room_state:{room_id}");
        self.get_cached_data(&key)
            .await?
            .ok_or_else(|| RedisError::from((redis::ErrorKind::ResponseError, "Key not found")))
    }

    pub async fn cache_public_spaces(
        &self,
        rooms: &Vec<RoomSummary>,
        ttl: u64,
    ) -> Result<(), RedisError> {
        self.cache_data("public_spaces", rooms, ttl).await
    }

    pub async fn get_cached_public_spaces(&self) -> Result<Vec<RoomSummary>, RedisError> {
        self.get_cached_data("public_spaces")
            .await?
            .ok_or_else(|| RedisError::from((redis::ErrorKind::ResponseError, "Key not found")))
    }

    pub async fn cache_room_state(
        &self,
        room_id: &str,
        state: &Vec<PublicRoom>,
        ttl: u64,
    ) -> Result<(), RedisError> {
        let key = format!("room_state:{room_id}");
        self.cache_data(&key, state, ttl).await
    }

    pub async fn cache_proxy_response(
        &self,
        key: &str,
        data: &[u8],
        ttl: u64,
    ) -> Result<(), RedisError> {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;
        let _: () = conn.set_ex(key, data, ttl).await?;
        Ok(())
    }

    pub async fn get_cached_proxy_response(&self, key: &str) -> Result<Vec<u8>, RedisError> {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;

        if !conn.exists(key).await? {
            return Err(RedisError::from((
                redis::ErrorKind::ResponseError,
                "Key not found",
            )));
        }

        conn.get(key).await
    }

    pub async fn cache_multiple<T>(&self, items: Vec<(&str, &T, u64)>) -> Result<(), RedisError>
    where
        T: Cacheable,
    {
        for (key, data, ttl) in items {
            self.cache_data(key, data, ttl).await?;
        }
        Ok(())
    }

    pub async fn delete_multiple(&self, keys: &[&str]) -> Result<(), RedisError> {
        for key in keys {
            self.delete_cached_data(key).await?;
        }
        Ok(())
    }
}
