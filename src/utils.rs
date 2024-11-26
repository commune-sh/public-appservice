use ruma::{RoomId, OwnedRoomId};

pub fn is_room_id_ok(room_id: &str, server_name: &str) -> Result<OwnedRoomId, String> {

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
