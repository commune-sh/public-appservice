use serde::Deserialize;
use std::{fs, process};


#[derive(Debug, Deserialize)]
pub struct Config {
    pub appservice: AppService,
    pub matrix: Matrix,
}

#[derive(Debug, Deserialize)]
pub struct AppService {
    pub port: u16,
    pub id: String,
    pub sender_localpart: String,
    pub access_token: String,
    pub hs_access_token: String,
    pub rules: AppServiceRules,
}

#[derive(Debug, Deserialize)]
pub struct AppServiceRules {
    pub auto_join: bool,
    pub invite_by_local_user: bool,
    pub federation_domain_whitelist: Vec<String>,
}

#[derive(Debug, Deserialize)]
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
