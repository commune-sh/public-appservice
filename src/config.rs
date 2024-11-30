use serde::{Serialize, Deserialize};
use std::{fs, process};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: Server,
    pub appservice: AppService,
    pub matrix: Matrix,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub port: u16,
    pub allow_origin: Option<Vec<String>>,
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

impl Config {
    pub fn new() -> Self {
        read()
    }

    pub fn print(&self) {
        println!("{:#?}", self);
    }
}

pub fn read() -> Config {
    let config_content = match fs::read_to_string("config.toml") {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Failed to read config.toml: {}", e);
            process::exit(1);
        }
    };
    
    match toml::from_str(&config_content) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Failed to parse config.toml: {}", e);
            process::exit(1);
        }
    }
}
