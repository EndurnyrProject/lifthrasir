//! Ordered chat room ingress: room traffic, plus the unit despawns and map changes
//! that take signs and membership away.

use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;
use net_contract::events::ChatRoomEvent;

use crate::dispatch::IncomingMessage;
use crate::zone::mapping::chat_room::chat_room_event;

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_chat_rooms(
    mut incoming: MessageReader<IncomingMessage>,
    mut out: MessageWriter<ChatRoomEvent>,
) {
    out.write_batch(
        incoming
            .read()
            .filter_map(|message| chat_room_event(message.body.clone())),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::WORLD;
    use crate::envelope::Body;
    use crate::proto::aesir::net;

    #[test]
    fn an_owner_handoff_keeps_its_order() {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<ChatRoomEvent>()
            .add_systems(Update, zone_drain_chat_rooms);
        let bodies = [
            Body::WaitingRoomRoleChanged(net::WaitingRoomRoleChanged {
                room_id: 9,
                char_id: 5,
                owner: true,
            }),
            Body::WaitingRoomRemoved(net::WaitingRoomRemoved { room_id: 9 }),
            Body::ChatLine(net::ChatLine::default()),
            Body::WaitingRoomInfo(net::WaitingRoomInfo {
                room_id: 9,
                owner_gid: 5,
                ..Default::default()
            }),
        ];
        let mut incoming = app.world_mut().resource_mut::<Messages<IncomingMessage>>();
        for body in bodies {
            incoming.write(IncomingMessage {
                channel: WORLD,
                body,
            });
        }
        app.update();

        let events: Vec<_> = app
            .world()
            .resource::<Messages<ChatRoomEvent>>()
            .iter_current_update_messages()
            .cloned()
            .collect();
        assert!(matches!(
            events[..],
            [
                ChatRoomEvent::OwnerChanged { char_id: 5, .. },
                ChatRoomEvent::SignRemoved { room_id: 9 },
                ChatRoomEvent::SignShown(_),
            ]
        ));
    }
}
