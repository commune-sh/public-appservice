use crate::config::Config;

use redis::{
    AsyncCommands,
    RedisError
};

use crate::rooms::PublicRoom;

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

    pub async fn cache_rooms(
        &self,
        rooms: &Vec<PublicRoom>,
        ttl: u64,
    ) -> Result<(), RedisError> {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;

        let serialized = serde_json::to_string(rooms).map_err(|e| {
            RedisError::from((
                redis::ErrorKind::IoError,
                "Serialization error",
                e.to_string(),
            ))
        })?;

        conn.set_ex("public_rooms", serialized, ttl).await
    }

    pub async fn get_cached_rooms(&self) -> Result<Vec<PublicRoom>, RedisError> {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;

        let data: String = conn.get("public_rooms").await?;
        serde_json::from_str(&data).map_err(|e| {
            RedisError::from((
                redis::ErrorKind::IoError,
                "Deserialization error",
                e.to_string(),
            ))
        })
    }

    pub async fn get_cached_room_state(
        &self,
        room_id: &str,
    ) -> Result<Vec<PublicRoom>, RedisError> {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;

        let key = format!("room_state:{}", room_id);
        let data: String = conn.get(key).await?;
        serde_json::from_str(&data).map_err(|e| {
            RedisError::from((
                redis::ErrorKind::IoError,
                "Deserialization error",
                e.to_string(),
            ))
        })
    }

    pub async fn cache_room_state(
        &self,
        room_id: &str,
        state: &Vec<PublicRoom>,
        ttl: u64,
    ) -> Result<(), RedisError> {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;

        let key = format!("room_state:{}", room_id);
        let serialized = serde_json::to_string(state).map_err(|e| {
            RedisError::from((
                redis::ErrorKind::IoError,
                "Serialization error",
                e.to_string(),
            ))
        })?;

        conn.set_ex(key, serialized, ttl).await
    }

}
