use crate::config::Config;

use redis::{
    AsyncCommands,
    RedisError
};

use serde::{Serialize, Deserialize};

use crate::rooms::PublicRoom;
use crate::appservice::RoomSummary;

#[derive(Debug, Clone)]
pub struct Cache {
    pub client: redis::Client,
}

impl Cache {

    pub async fn new(config: &Config) -> Result<Self, anyhow::Error> {

        let url = format!("redis://{}/{}", config.redis.url, config.redis.db);
        let client = redis::Client::open(url)?;

        Ok(Self { client })

    }

    pub async fn cache_data<T>(&self, key: &str, data: &T, ttl: u64) -> Result<(), RedisError>
    where
        T: Serialize,
    {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;

        let serialized = serde_json::to_string(data).map_err(|e| {
            RedisError::from((
                redis::ErrorKind::IoError,
                "Serialization error",
                e.to_string(),
            ))
        })?;

        conn.set_ex(key, serialized, ttl).await
    }

    pub async fn get_cached_data<T>(&self, key: &str) -> Result<T, RedisError>
    where
        T: for<'a> Deserialize<'a>,
    {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;

        let data: String = conn.get(key).await?;
        serde_json::from_str(&data).map_err(|e| {
            RedisError::from((
                redis::ErrorKind::IoError,
                "Deserialization error",
                e.to_string(),
            ))
        })
    }

    pub async fn cache_rooms(
        &self,
        rooms: &Vec<PublicRoom>,
        ttl: u64,
    ) -> Result<(), RedisError> {

        self.cache_data("public_rooms", rooms, ttl).await

    }

    pub async fn get_cached_rooms(&self) -> Result<Vec<PublicRoom>, RedisError> {
        self.get_cached_data("public_rooms").await
    }

    pub async fn get_cached_room_state(
        &self,
        room_id: &str,
    ) -> Result<Vec<PublicRoom>, RedisError> {

        let key = format!("room_state:{}", room_id);
        self.get_cached_data(&key).await

    }

    pub async fn cache_public_spaces(
        &self,
        rooms: &Vec<RoomSummary>,
        ttl: u64,
    ) -> Result<(), RedisError> {
        self.cache_data("public_spaces", rooms, ttl).await
    }

    pub async fn get_cached_public_spaces(&self) -> Result<Vec<RoomSummary>, RedisError> {
        self.get_cached_data("public_spaces").await
    }


    pub async fn cache_room_state(
        &self,
        room_id: &str,
        state: &Vec<PublicRoom>,
        ttl: u64,
    ) -> Result<(), RedisError> {

        let key = format!("room_state:{}", room_id);
        self.cache_data(&key, state, ttl).await

    }

    pub async fn cache_proxy_response(
        &self, 
        key: &str, 
        data: &[u8], 
        ttl: u64
    ) -> Result<(), RedisError> {

        let mut conn = self.client.get_multiplexed_tokio_connection().await?;
        conn.set_ex(key, data, ttl).await
    }

    pub async fn get_cached_proxy_response(
        &self, 
        key: &str
    ) -> Result<Vec<u8>, RedisError> {

        let mut conn = self.client.get_multiplexed_tokio_connection().await?;

        // check if the key exists
        if !conn.exists(key).await? {
            return Err(RedisError::from((
                redis::ErrorKind::ResponseError,
                "Key not found",
            )));
        }

        conn.get(key).await
    }

}
