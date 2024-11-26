use ruma::{RoomId, OwnedRoomId};

pub fn is_room_id_ok(room_id: &str, server_name: &str) -> Result<OwnedRoomId, String> {

    println!("Checking room ID and server_name: {} {}", room_id, server_name);

    match RoomId::parse(room_id) {
        Ok(id) => {
            let parts: Vec<&str> = room_id.split(':').collect();
            if parts.len() != 2 || !room_id.starts_with('!') {
                println!("yikes");
                return Err("Room ID must start with '!' and contain a single ':'".to_string());
            }
            
            let domain = parts[1];
            println!("Domain: {}", domain);
            if domain.is_empty() {
                return Err("Room ID must have a valid domain part".to_string());
            }

            // Check if the domain part of the room ID matches the server server_name   
            if domain != server_name {
                return Err(format!("Room ID domain part does not match server_name: {} != {}", domain, server_name));
            }

            Ok(id)
        }
        Err(err) => Err(format!("Failed to parse Room ID: {}", err)),
    }
}
