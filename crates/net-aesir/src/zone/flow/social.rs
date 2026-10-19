use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::social::{chat_message, emotion, name_response};
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;
use net_contract::events::{ChatHeard, EmoteShown, EntityNamed};

/// Drains social bodies (chat, entity names, and emotes). All ride the world
/// channel, but the match is on the `Body` variant for consistency with the
/// other channel-spanning interaction drains.
#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_social(
    mut incoming: MessageReader<IncomingMessage>,
    mut chat: MessageWriter<ChatHeard>,
    mut named: MessageWriter<EntityNamed>,
    mut emote: MessageWriter<EmoteShown>,
) {
    for msg in incoming.read() {
        match msg.body.clone() {
            Body::ChatMessage(c) => {
                chat.write(chat_message(c));
            }
            Body::NameResponse(n) => {
                named.write(name_response(n));
            }
            Body::Emotion(e) => {
                emote.write(emotion(e));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::WORLD;
    use crate::proto::aesir::net;

    fn drain(bodies: Vec<(u8, Body)>) -> App {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<ChatHeard>()
            .add_message::<EntityNamed>()
            .add_message::<EmoteShown>()
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

    #[test]
    fn emotion_produces_one_emote_shown() {
        let app = drain(vec![(
            WORLD,
            Body::Emotion(net::Emotion {
                gid: 150001,
                r#type: 4,
            }),
        )]);

        let emote = app.world().resource::<Messages<EmoteShown>>();
        let events: Vec<_> = emote.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].gid, 150001);
        assert_eq!(events[0].emote_type, 4);
    }
}
