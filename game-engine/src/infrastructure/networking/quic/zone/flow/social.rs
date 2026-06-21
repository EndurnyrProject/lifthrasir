use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::social::{chat_message, name_response};
use crate::infrastructure::networking::quic::dispatch::IncomingMessage;
use crate::infrastructure::networking::quic::envelope::Body;
use crate::infrastructure::networking::zone_messages::{ChatHeard, EntityNamed};

/// Drains social bodies (chat and entity names). Both ride the world channel,
/// but the match is on the `Body` variant for consistency with the other
/// channel-spanning interaction drains.
#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_social(
    mut incoming: MessageReader<IncomingMessage>,
    mut chat: MessageWriter<ChatHeard>,
    mut named: MessageWriter<EntityNamed>,
) {
    for msg in incoming.read() {
        match msg.body.clone() {
            Body::ChatMessage(c) => {
                chat.write(chat_message(c));
            }
            Body::NameResponse(n) => {
                named.write(name_response(n));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::networking::quic::channels::WORLD;
    use crate::infrastructure::networking::quic::proto::aesir::net;

    fn drain(bodies: Vec<(u8, Body)>) -> App {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<ChatHeard>()
            .add_message::<EntityNamed>()
            .add_systems(Update, zone_drain_social);

        let mut incoming = app.world_mut().resource_mut::<Messages<IncomingMessage>>();
        for (channel, body) in bodies {
            incoming.write(IncomingMessage { channel, body });
        }
        app.update();
        app
    }

    #[test]
    fn chat_message_produces_one_chat_heard() {
        let app = drain(vec![(
            WORLD,
            Body::ChatMessage(net::ChatMessage {
                gid: 150001,
                message: "hello".into(),
            }),
        )]);

        let chat = app.world().resource::<Messages<ChatHeard>>();
        let events: Vec<_> = chat.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].message, "hello");
    }

    #[test]
    fn name_response_produces_one_entity_named() {
        let app = drain(vec![(
            WORLD,
            Body::NameResponse(net::NameResponse {
                gid: 150001,
                name: "Alice".into(),
                ..Default::default()
            }),
        )]);

        let named = app.world().resource::<Messages<EntityNamed>>();
        let events: Vec<_> = named.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "Alice");
    }
}
