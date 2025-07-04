use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AuthMetadata {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub registration_endpoint: String,
}

pub async fn get_auth_metadata(homeserver: &str) -> Result<AuthMetadata, anyhow::Error> {
    let url = format!("{homeserver}/_matrix/client/unstable/org.matrix.msc2965/auth_metadata");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .connect_timeout(Duration::from_secs(3))
        .build()?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| {
            anyhow::anyhow!("Failed to fetch auth metadata from {}: {}", url, e)
        })?;

    let metadata = response
        .json::<AuthMetadata>()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to parse metadata: {}", e))?;

    Ok(metadata)
}
