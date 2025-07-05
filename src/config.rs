use serde::{Deserialize, Serialize};
use std::path::Path;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server: Server,
    pub appservice: AppService,
    pub matrix: Matrix,
    #[serde(default)]
    pub redis: Redis,
    pub db: DB,
    #[serde(default)]
    pub cache: Cache,
    #[serde(default)]
    pub public_rooms: PublicRooms,
    #[serde(default)]
    pub spaces: Spaces,
    pub logging: Option<Logging>,
    #[serde(default)]
    pub search: Search,
    #[serde(default)]
    pub sentry: Option<Sentry>,
    #[serde(default)]
    pub metrics: Metrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    #[serde(default = "default_port")]
    pub port: u16,
    pub allow_origin: Option<Vec<String>>,
}

impl Default for Server {
    fn default() -> Self {
        Self {
            port: default_port(),
            allow_origin: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppService {
    pub id: String,
    pub sender_localpart: String,
    pub access_token: String,
    pub hs_access_token: String,
    #[serde(default)]
    pub rules: AppServiceRules,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppServiceRules {
    #[serde(default)]
    pub auto_join: bool,
    #[serde(default)]
    pub invite_by_local_user: bool,
    #[serde(default)]
    pub federation_domain_whitelist: Vec<String>,
}

impl Default for AppServiceRules {
    fn default() -> Self {
        Self {
            auto_join: false,
            invite_by_local_user: false,
            federation_domain_whitelist: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Matrix {
    pub homeserver: String,
    pub server_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Redis {
    #[serde(default = "default_redis_url")]
    pub url: String,
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl: u64,
}

impl Default for Redis {
    fn default() -> Self {
        Self {
            url: default_redis_url(),
            pool_size: default_pool_size(),
            timeout_secs: default_timeout_secs(),
            cache_ttl: default_cache_ttl(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cache {
    #[serde(default)]
    pub requests: CacheOptions,
    #[serde(default)]
    pub public_rooms: CacheOptions,
    #[serde(default)]
    pub room_state: CacheOptions,
    #[serde(default)]
    pub messages: CacheOptions,
    #[serde(default)]
    pub media: CacheOptions,
    #[serde(default)]
    pub search: CacheOptions,
}

impl Default for Cache {
    fn default() -> Self {
        Self {
            requests: CacheOptions::default(),
            public_rooms: CacheOptions::default(),
            room_state: CacheOptions::default(),
            messages: CacheOptions::default(),
            media: CacheOptions::default(),
            search: CacheOptions::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheOptions {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_cache_ttl")]
    pub expire_after: u64,
}

impl Default for CacheOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            expire_after: default_cache_ttl(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DB {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicRooms {
    #[serde(default)]
    pub curated: bool,
    #[serde(default)]
    pub include_rooms: Vec<String>,
}

impl Default for PublicRooms {
    fn default() -> Self {
        Self {
            curated: false,
            include_rooms: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spaces {
    #[serde(default)]
    pub default: Vec<String>,
    #[serde(default)]
    pub include_all: bool,
    #[serde(default)]
    pub cache: bool,
    #[serde(default = "default_spaces_ttl")]
    pub ttl: u64,
}

impl Default for Spaces {
    fn default() -> Self {
        Self {
            default: Vec::new(),
            include_all: false,
            cache: false,
            ttl: default_spaces_ttl(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Logging {
    pub directory: String,
    pub filename: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Sentry {
    pub enabled: bool,
    pub dsn: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Metrics {
    pub enabled: bool,
    pub port: u16,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Search {
    #[serde(default)]
    pub disabled: bool,
}

fn default_port() -> u16 {
    8989
}

fn default_redis_url() -> String {
    "127.0.0.1:6379/0".to_string()
}

fn default_pool_size() -> u32 {
    10
}

fn default_timeout_secs() -> u64 {
    5
}

fn default_cache_ttl() -> u64 {
    300
}

fn default_spaces_ttl() -> u64 {
    3600
}

impl Config {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, anyhow::Error> {
        let path = path.as_ref();

        let config_content = fs::read_to_string(path)?;

        let config = toml::from_str(&config_content)?;

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_with_minimal_toml() {
        let toml_content = r#"
            [appservice]
            id = "test"
            sender_localpart = "bot"
            access_token = "token"
            hs_access_token = "hs_token"

            [matrix]
            homeserver = "http://localhost:8008"
            server_name = "test.local"
        "#;

        let config: Config = toml::from_str(toml_content).expect("Should parse minimal config");

        assert_eq!(config.server.port, 8989);
        assert_eq!(config.redis.pool_size, 10);
        assert!(!config.cache.requests.enabled);
        assert!(!config.public_rooms.curated);
    }
}
