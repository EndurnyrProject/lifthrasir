use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::skill_units::{
    skill_unit_despawn, skill_unit_snapshot, skill_unit_spawn, skill_unit_update,
};
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;
use net_contract::events::{
    SkillUnitDespawned, SkillUnitSnapshotReceived, SkillUnitSpawned, SkillUnitUpdated,
};

/// Drains the persistent skill-unit lifecycle (Storm Gust groups, Ice Wall
/// cells, ...) into domain events. `EstimationResult` (tag 159) is out of
/// scope and left undecoded.
#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_skill_units(
    mut incoming: MessageReader<IncomingMessage>,
    mut snapshot: MessageWriter<SkillUnitSnapshotReceived>,
    mut spawned: MessageWriter<SkillUnitSpawned>,
    mut updated: MessageWriter<SkillUnitUpdated>,
    mut despawned: MessageWriter<SkillUnitDespawned>,
) {
    for msg in incoming.read() {
        match msg.body.clone() {
            Body::SkillUnitSnapshot(s) => {
                snapshot.write(skill_unit_snapshot(s));
            }
            Body::SkillUnitSpawn(s) => match skill_unit_spawn(s) {
                Some(event) => {
                    spawned.write(event);
                }
                None => warn!("dropping SkillUnitSpawn without a group payload"),
            },
            Body::SkillUnitUpdate(u) => {
                updated.write(skill_unit_update(u));
            }
            Body::SkillUnitDespawn(d) => {
                despawned.write(skill_unit_despawn(d));
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

    fn app_with_drain() -> App {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<SkillUnitSnapshotReceived>()
            .add_message::<SkillUnitSpawned>()
            .add_message::<SkillUnitUpdated>()
            .add_message::<SkillUnitDespawned>()
            .add_systems(Update, zone_drain_skill_units);
        app
    }

    fn write_incoming(app: &mut App, body: Body) {
        app.world_mut()
            .resource_mut::<Messages<IncomingMessage>>()
            .write(IncomingMessage {
                channel: GAMEPLAY,
                body,
            });
    }

    #[test]
    fn snapshot_drains_to_event() {
        let mut app = app_with_drain();
        write_incoming(
            &mut app,
            Body::SkillUnitSnapshot(net::SkillUnitSnapshot {
                server_tick: 100,
                groups: vec![net::SkillUnitGroupState {
                    group_id: 1,
                    ..Default::default()
                }],
            }),
        );
        app.update();

        let events = app
            .world()
            .resource::<Messages<SkillUnitSnapshotReceived>>();
        let drained: Vec<_> = events.iter_current_update_messages().collect();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].groups.len(), 1);
        assert_eq!(drained[0].groups[0].group_id, 1);
    }

    #[test]
    fn spawn_drains_to_event() {
        let mut app = app_with_drain();
        write_incoming(
            &mut app,
            Body::SkillUnitSpawn(net::SkillUnitSpawn {
                group: Some(net::SkillUnitGroupState {
                    group_id: 7,
                    ..Default::default()
                }),
            }),
        );
        app.update();

        let events = app.world().resource::<Messages<SkillUnitSpawned>>();
        let drained: Vec<_> = events.iter_current_update_messages().collect();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].group.group_id, 7);
    }

    #[test]
    fn spawn_without_group_is_dropped_not_written() {
        let mut app = app_with_drain();
        write_incoming(
            &mut app,
            Body::SkillUnitSpawn(net::SkillUnitSpawn { group: None }),
        );
        app.update();

        let events = app.world().resource::<Messages<SkillUnitSpawned>>();
        assert_eq!(events.iter_current_update_messages().count(), 0);
    }

    #[test]
    fn update_drains_to_event() {
        let mut app = app_with_drain();
        write_incoming(
            &mut app,
            Body::SkillUnitUpdate(net::SkillUnitUpdate {
                group_id: 7,
                cell_id: 700,
                hp: 50,
                max_hp: 100,
                hp_delta: -50,
                reason: net::SkillUnitUpdateReason::Damage as i32,
                ..Default::default()
            }),
        );
        app.update();

        let events = app.world().resource::<Messages<SkillUnitUpdated>>();
        let drained: Vec<_> = events.iter_current_update_messages().collect();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].group_id, 7);
        assert_eq!(drained[0].hp, 50);
    }

    #[test]
    fn despawn_drains_to_event() {
        let mut app = app_with_drain();
        write_incoming(
            &mut app,
            Body::SkillUnitDespawn(net::SkillUnitDespawn {
                group_id: 7,
                cell_ids: vec![700, 701],
                reason: net::SkillUnitDespawnReason::Expired as i32,
                ..Default::default()
            }),
        );
        app.update();

        let events = app.world().resource::<Messages<SkillUnitDespawned>>();
        let drained: Vec<_> = events.iter_current_update_messages().collect();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].group_id, 7);
        assert_eq!(drained[0].cell_ids, vec![700, 701]);
    }
}
