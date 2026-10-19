use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;
use net_contract::events::ViewpointChanged;

use super::super::mapping::viewpoint::viewpoint;
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_viewpoints(
    mut incoming: MessageReader<IncomingMessage>,
    mut out: MessageWriter<ViewpointChanged>,
) {
    for msg in incoming.read() {
        let Body::Viewpoint(m) = msg.body.clone() else {
            continue;
        };
        if let Some(ev) = viewpoint(m) {
            out.write(ev);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::GAMEPLAY;
    use crate::proto::aesir::net;

    #[test]
    fn viewpoint_drains_to_event() {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<ViewpointChanged>()
            .add_systems(Update, zone_drain_viewpoints);

        app.world_mut()
            .resource_mut::<Messages<IncomingMessage>>()
            .write(IncomingMessage {
                channel: GAMEPLAY,
                body: Body::Viewpoint(net::Viewpoint {
                    npc_id: 99,
                    r#type: 1,
                    id: 3,
                    x: 10,
                    y: 20,
                    color: 0x00FF0000,
                }),
            });
        app.update();

        let events = app.world().resource::<Messages<ViewpointChanged>>();
        let drained: Vec<_> = events.iter_current_update_messages().collect();
        assert_eq!(drained.len(), 1);
        let ViewpointChanged::Show { id, x, y, .. } = drained[0] else {
            panic!("expected Show, got {:?}", drained[0]);
        };
        assert_eq!((*id, *x, *y), (3, 10, 20));
    }

    #[test]
    fn non_viewpoint_bodies_are_ignored() {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<ViewpointChanged>()
            .add_systems(Update, zone_drain_viewpoints);

        app.world_mut()
            .resource_mut::<Messages<IncomingMessage>>()
            .write(IncomingMessage {
                channel: GAMEPLAY,
                body: Body::SoundEffect(net::SoundEffect {
                    name: "effect\\door.wav".into(),
                    r#type: 0,
                }),
            });
        app.update();

        let events = app.world().resource::<Messages<ViewpointChanged>>();
        assert_eq!(events.iter_current_update_messages().count(), 0);
    }
}
