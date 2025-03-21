use regex::Regex;
use once_cell::sync::Lazy;

use ruma::{
    RoomId, 
    OwnedRoomId
};

pub fn room_id_valid(room_id: &str, server_name: &str) -> Result<OwnedRoomId, anyhow::Error> {

    let parsed_id = RoomId::parse(room_id)?;

    if let Some(domain) = parsed_id.server_name() {
        if domain != server_name {
            return Err(anyhow::anyhow!("Room ID does not match server name"));
        }
    }

    Ok(parsed_id)

}

static SLUG_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"[^a-zA-Z0-9]+").unwrap());


pub fn slugify(s: &str) -> String {
    SLUG_REGEX.replace_all(s, "-").to_string().to_lowercase()
}
