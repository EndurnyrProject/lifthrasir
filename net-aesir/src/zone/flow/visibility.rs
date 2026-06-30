use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::visibility::{unit_despawn, unit_spawn};
use crate::channels::WORLD;
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;
use net_contract::events::{UnitEntered, UnitLeft};

/// Drains the world channel for entity-visibility spawn and despawn bodies.
#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_visibility(
    mut incoming: MessageReader<IncomingMessage>,
    mut entered: MessageWriter<UnitEntered>,
    mut left: MessageWriter<UnitLeft>,
) {
    for msg in incoming.read() {
        if msg.channel != WORLD {
            continue;
        }
        match msg.body.clone() {
            Body::UnitSpawn(s) => {
                entered.write(unit_spawn(s));
            }
            Body::UnitDespawn(d) => {
                left.write(unit_despawn(d));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::GAMEPLAY;
    use crate::proto::aesir::net;

    fn drain_world(bodies: Vec<(u8, Body)>) -> App {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<UnitEntered>()
            .add_message::<UnitLeft>()
            .add_systems(Update, zone_drain_visibility);

        let mut incoming = app.world_mut().resource_mut::<Messages<IncomingMessage>>();
        for (channel, body) in bodies {
            incoming.write(IncomingMessage { channel, body });
        }
        app.update();
        app
    }

    #[test]
    fn unit_spawn_on_world_produces_one_unit_entered() {
        let spawn = net::UnitSpawn {
            gid: 150001,
            name: "Alice".into(),
            ..Default::default()
        };
        let app = drain_world(vec![(WORLD, Body::UnitSpawn(spawn))]);

        let entered = app.world().resource::<Messages<UnitEntered>>();
        let events: Vec<_> = entered.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].gid, 150001);
        assert_eq!(events[0].name, "Alice");
    }

    #[test]
    fn unit_spawn_off_world_channel_is_skipped() {
        let spawn = net::UnitSpawn {
            gid: 150001,
            ..Default::default()
        };
        let app = drain_world(vec![(GAMEPLAY, Body::UnitSpawn(spawn))]);

        let entered = app.world().resource::<Messages<UnitEntered>>();
        assert_eq!(entered.iter_current_update_messages().count(), 0);
    }
}
