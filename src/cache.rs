use crate::config::Config;

pub struct Cache {
    pub client: redis::Client,
}

impl Cache {
    pub async fn new(config: &Config) -> Result<Self, anyhow::Error> {
        let url = format!("redis://{}", config.redis.url);
        let client = redis::Client::open(url)?;
        Ok(Self { client })
    }
}
