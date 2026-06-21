use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::stats::{
    param_change, resurrect, sprite_change, stat_up_result, unit_hp,
};
use crate::infrastructure::networking::quic::dispatch::IncomingMessage;
use crate::infrastructure::networking::quic::envelope::Body;
use crate::infrastructure::networking::zone_messages::{
    ParamChanged, StatRaised, UnitHpChanged, UnitResurrected, UnitSpriteChanged,
};

/// Drains stat bodies. These span the gameplay and world channels, so the match
/// is on the `Body` variant directly, not the channel. `Respawn` is client→server
/// (CZ_RESTART) and never inbound, so it is not drained here.
#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_stats(
    mut incoming: MessageReader<IncomingMessage>,
    mut param: MessageWriter<ParamChanged>,
    mut hp: MessageWriter<UnitHpChanged>,
    mut raised: MessageWriter<StatRaised>,
    mut sprite: MessageWriter<UnitSpriteChanged>,
    mut resurrected: MessageWriter<UnitResurrected>,
) {
    for msg in incoming.read() {
        match msg.body.clone() {
            Body::ParamChange(p) => {
                param.write(param_change(p));
            }
            Body::UnitHp(u) => {
                hp.write(unit_hp(u));
            }
            Body::StatUpResult(s) => {
                raised.write(stat_up_result(s));
            }
            Body::SpriteChange(s) => {
                sprite.write(sprite_change(s));
            }
            Body::Resurrect(r) => {
                resurrected.write(resurrect(r));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::networking::quic::channels::{GAMEPLAY, WORLD};
    use crate::infrastructure::networking::quic::proto::aesir::net;

    fn drain(bodies: Vec<(u8, Body)>) -> App {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<ParamChanged>()
            .add_message::<UnitHpChanged>()
            .add_message::<StatRaised>()
            .add_message::<UnitSpriteChanged>()
            .add_message::<UnitResurrected>()
            .add_systems(Update, zone_drain_stats);

        let mut incoming = app.world_mut().resource_mut::<Messages<IncomingMessage>>();
        for (channel, body) in bodies {
            incoming.write(IncomingMessage { channel, body });
        }
        app.update();
        app
    }

    #[test]
    fn param_change_on_gameplay_produces_one_param_changed() {
        let app = drain(vec![(
            GAMEPLAY,
            Body::ParamChange(net::ParamChange {
                var_id: 5,
                value: 4_294_967_296,
            }),
        )]);

        let param = app.world().resource::<Messages<ParamChanged>>();
        let events: Vec<_> = param.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].var, 5);
        assert_eq!(events[0].value, 4_294_967_296);
    }

    #[test]
    fn unit_hp_on_world_produces_one_unit_hp_changed() {
        let app = drain(vec![(
            WORLD,
            Body::UnitHp(net::UnitHp {
                id: 150001,
                hp: 3000,
                max_hp: 4200,
            }),
        )]);

        let hp = app.world().resource::<Messages<UnitHpChanged>>();
        let events: Vec<_> = hp.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].gid, 150001);
    }
}
