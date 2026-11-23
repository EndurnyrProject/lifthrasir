//! Ordered quest log ingress: one stream, so a removal and the addition that
//! replaces it reach the client in the order the server sent them.

use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;
use net_contract::events::QuestEvent;

use crate::dispatch::IncomingMessage;
use crate::zone::mapping::quest::quest_event;

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_quests(
    mut incoming: MessageReader<IncomingMessage>,
    mut out: MessageWriter<QuestEvent>,
) {
    out.write_batch(
        incoming
            .read()
            .filter_map(|message| quest_event(message.body.clone())),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::GAMEPLAY;
    use crate::envelope::Body;
    use crate::proto::aesir::net;

    #[test]
    fn a_replaced_quest_keeps_its_order() {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<QuestEvent>()
            .add_systems(Update, zone_drain_quests);
        let bodies = [
            Body::QuestRemoved(net::QuestRemoved { quest_id: 7 }),
            Body::ChatLine(net::ChatLine::default()),
            Body::QuestAdded(net::QuestAdded {
                quest: Some(net::QuestEntry {
                    quest_id: 7,
                    state: 1,
                    ..Default::default()
                }),
            }),
        ];
        let mut incoming = app.world_mut().resource_mut::<Messages<IncomingMessage>>();
        for body in bodies {
            incoming.write(IncomingMessage {
                sequence: 0,
                channel: GAMEPLAY,
                body,
            });
        }
        app.update();

        let events: Vec<_> = app
            .world()
            .resource::<Messages<QuestEvent>>()
            .iter_current_update_messages()
            .cloned()
            .collect();
        assert!(matches!(
            events[..],
            [QuestEvent::Removed { quest_id: 7 }, QuestEvent::Added(_),]
        ));
    }
}
