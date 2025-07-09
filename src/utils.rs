use once_cell::sync::Lazy;
use regex::Regex;

use ruma::{OwnedRoomId, RoomId};

pub fn room_id_valid(room_id: &str, server_name: &str) -> Result<OwnedRoomId, anyhow::Error> {
    let parsed_id = RoomId::parse(room_id)?;

    if let Some(domain) = parsed_id.server_name() {
        if domain != server_name {
            return Err(anyhow::anyhow!("Room ID does not match server name"));
        }
    }

    Ok(parsed_id)
}

static SLUG_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[^a-zA-Z0-9]+").expect("Failed to compile regex for slugify"));

pub fn slugify(s: &str) -> String {
    SLUG_REGEX.replace_all(s, "-").to_string().to_lowercase()
}

pub fn room_alias_like(alias: &str) -> bool {
    let parts: Vec<&str> = alias.split(':').collect();
    parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty()
}
