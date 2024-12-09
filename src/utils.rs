use regex::Regex;
use once_cell::sync::Lazy;

use ruma::{
    RoomId, 
    OwnedRoomId
};

pub fn room_id_valid(room_id: &str, server_name: &str) -> Result<OwnedRoomId, String> {

    match RoomId::parse(room_id) {

        Ok(id) => {

            if !room_id.starts_with('!') {
                return Err("Room ID must start with '!'".to_string());
            }

            let pos = room_id.find(':')
                .ok_or_else(|| "Room ID must contain a ':'".to_string())?;

            let domain = &room_id[pos + 1..];

            if domain.is_empty() {
                return Err("Room ID must have a valid domain part".to_string());
            }

            if domain != server_name {
                return Err(format!("Room ID domain part does not match server_name: {} != {}", domain, server_name));
            }

            Ok(id)
        }

        Err(err) => Err(format!("Failed to parse Room ID: {}", err)),
    }
}

static SLUG_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"[^a-zA-Z0-9]+").unwrap());


pub fn slugify(s: &str) -> String {
    SLUG_REGEX.replace_all(s, "-").to_string().to_lowercase()
}
