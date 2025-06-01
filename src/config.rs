use crate::constants::DEFAULT_CONFIG_PATH;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, process::ExitCode};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: Server,
    pub appservice: AppService,
    pub matrix: Matrix,
    pub redis: Redis,
    pub cache: Cache,
    pub public_rooms: PublicRooms,
    pub tracing: Tracing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub port: u16,
    pub allow_origin: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tracing {
    pub filter: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppService {
    pub id: String,
    pub sender_localpart: String,
    pub access_token: String,
    pub hs_access_token: String,
    pub rules: AppServiceRules,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppServiceRules {
    pub auto_join: bool,
    pub invite_by_local_user: bool,
    pub federation_domain_whitelist: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Matrix {
    pub homeserver: String,
    pub server_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Redis {
    pub url: String,
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl: u64,
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

fn default_false() -> bool {
    false
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cache {
    pub public_rooms: CacheOptions,
    pub room_state: CacheOptions,
    pub messages: CacheOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheOptions {
    #[serde(default = "default_false")]
    pub enabled: bool,
    #[serde(default = "default_cache_ttl")]
    pub expire_after: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicRooms {
    pub curated: bool,
    pub include_rooms: Vec<String>,
}

impl Config {
    pub fn new(path: Option<PathBuf>) -> Result<Self, ExitCode> {
        let path = path.map(Ok).unwrap_or_else(Self::search)?;

        let content = fs::read_to_string(path).map_err(|error| {
            eprintln!("Failed to read config: {error}",);

            ExitCode::FAILURE
        })?;

        toml::from_str(&content).map_err(|error| {
            eprintln!("Failed to parse config: {error}",);

            ExitCode::FAILURE
        })
    }

    fn search() -> Result<PathBuf, ExitCode> {
        let dirs = xdg::BaseDirectories::new();

        dirs.find_config_file(&*DEFAULT_CONFIG_PATH).ok_or_else(|| {
            eprintln!(
                "Failed to find config at path: {}",
                DEFAULT_CONFIG_PATH.display()
            );

            ExitCode::FAILURE
        })
    }
}
