use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;
use net_contract::events::QuestMarkChanged;

use super::super::mapping::quest_mark::quest_mark;
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_quest_marks(
    mut incoming: MessageReader<IncomingMessage>,
    mut out: MessageWriter<QuestMarkChanged>,
) {
    for msg in incoming.read() {
        let Body::QuestInfoIcon(m) = msg.body.clone() else {
            continue;
        };
        out.write(quest_mark(m));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::GAMEPLAY;
    use crate::proto::aesir::net;

    #[test]
    fn quest_info_icon_drains_to_one_event() {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<QuestMarkChanged>()
            .add_systems(Update, zone_drain_quest_marks);

        app.world_mut()
            .resource_mut::<Messages<IncomingMessage>>()
            .write(IncomingMessage {
                sequence: 0,
                channel: GAMEPLAY,
                body: Body::SoundEffect(net::SoundEffect {
                    name: "effect\\door.wav".into(),
                    r#type: 0,
                }),
            });
        app.update();
        assert_eq!(
            app.world()
                .resource::<Messages<QuestMarkChanged>>()
                .iter_current_update_messages()
                .count(),
            0
        );

        app.world_mut()
            .resource_mut::<Messages<IncomingMessage>>()
            .write(IncomingMessage {
                sequence: 1,
                channel: GAMEPLAY,
                body: Body::QuestInfoIcon(net::QuestInfoIcon {
                    npc_id: 7,
                    x: 120,
                    y: 85,
                    icon: 0,
                    color: 1,
                }),
            });
        app.update();
        let events = app.world().resource::<Messages<QuestMarkChanged>>();
        let drained: Vec<_> = events.iter_current_update_messages().copied().collect();
        assert_eq!(
            drained,
            [QuestMarkChanged::Show {
                npc_id: 7,
                x: 120,
                y: 85,
                icon: 0,
                color: 1,
            }]
        );
    }
}
