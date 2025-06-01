use std::sync::Arc;

use axum::extract::State;
use ruma::{
    api::appservice::event::push_events,
    events::{
        room::{
            history_visibility::HistoryVisibility,
            member::{MembershipState, RoomMemberEvent},
        }, AnyStateEvent,
    },
};

use crate::{
    api::OriginalCommunePublicRoomEvent,
    error::serve::Result,
    Application,
};

pub async fn push_events_route(
    State(app): State<Arc<Application>>,
    request: push_events::v1::Request,
) -> Result<push_events::v1::Response> {
    for event in request.events {
        // TODO: log events?

        // If auto-join is enabled, join rooms with world_readable history visibility
        if app.config.appservice.rules.auto_join {
            if let Ok(AnyStateEvent::RoomHistoryVisibility(event)) =
                serde_json::from_str::<AnyStateEvent>(event.json().get())
            {
                if event.history_visibility() == &HistoryVisibility::WorldReadable {
                    let room_id = event.room_id().to_owned();

                    println!("History Visibility: World Readable");

                    tracing::info!("Joining room: {}", room_id);

                    let _ = app.appservice.join_room(room_id).await;
                }
            }
        }

        // Match commune.room.public types

        // let room_id = event["room_id"].as_str();
        // let event_type = event["type"].as_str();
        // let public = event["content"]["public"].as_bool();

        // match room_id {
        //     Some(room_id) => {
        //         println!("Room ID: {}", room_id);
        //         let room_id = RoomId::parse(room_id);
        //         match (event_type, public) {
        //             (Some("commune.room.public"), Some(true)) => {
        //                 info!("Joining room: {}", room_id);
        //                 let _ = state.appservice.join_room(room_id).await;
        //             }
        //             (Some("commune.room.public"), Some(false)) => {
        //                 println!("Leave room");
        //             }
        //             _ => {}
        //         }
        //     }
        //     None => {}
        // }

        if let Ok(event) =
            serde_json::from_str::<OriginalCommunePublicRoomEvent>(event.json().get())
        {
            tracing::info!("Commune Public room event.");

            let room_id = event.room_id.clone();

            if event.content.public {
                tracing::info!("Joining room: {}", room_id);

                let _ = app.appservice.join_room(room_id).await;
            } else {
                tracing::info!("Leaving room: {}", room_id);

                let _ = app.appservice.leave_room(room_id).await;
            }
        }

        let member_event =
            if let Ok(event) = serde_json::from_str::<RoomMemberEvent>(event.json().get()) {
                event
            } else {
                continue;
            };

        print!("Member Event: {:#?}", member_event);

        let room_id = member_event.room_id().to_owned();
        let membership = member_event.membership().to_owned();
        let servername = member_event.room_id().server_name();

        match servername {
            Some(servername) => {
                let whitelist = &app.config.appservice.rules.federation_domain_whitelist;

                let allowed = whitelist.iter().any(|s| servername.host().ends_with(s));

                if servername.as_str() != app.config.matrix.server_name && allowed {
                    // Ignore events for rooms on other servers, if configured to local homeserver
                    // users
                    if app.config.appservice.rules.invite_by_local_user {
                        tracing::info!("Ignoring event for room on different server: {servername}",);

                        continue;
                    }
                }
            }
            None => {
                tracing::info!("Ignoring event for room with no server name");

                continue;
            }
        }

        // Ignore membership events for other users
        let invited_user = member_event.state_key().to_owned();

        if invited_user != app.appservice.user_id() {
            tracing::info!("Ignoring event for user: {}", invited_user);

            continue;
        }

        match membership {
            MembershipState::Invite => {
                tracing::info!("Joining room: {}", room_id);

                let _ = app.appservice.join_room(room_id).await;
            }
            MembershipState::Leave => {
                let _ = app.appservice.leave_room(room_id).await;
            }
            MembershipState::Ban => {
                tracing::info!("Banned from room: {}", room_id);

                let _ = app.appservice.leave_room(room_id).await;
                //state.appservice.leave_room(room_id).await;
            }
            _ => {}
        }
    }

    Ok(push_events::v1::Response::new())
}
