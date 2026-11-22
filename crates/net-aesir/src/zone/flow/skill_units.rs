use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::skill_units::{
    skill_unit_despawn, skill_unit_move, skill_unit_snapshot, skill_unit_spawn, skill_unit_update,
};
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;
use net_contract::events::{
    SkillUnitDespawned, SkillUnitMoved, SkillUnitSnapshotReceived, SkillUnitSpawned,
    SkillUnitUpdated,
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
    mut moved: MessageWriter<SkillUnitMoved>,
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
            Body::SkillUnitMove(m) => {
                moved.write(skill_unit_move(m));
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
            .add_message::<SkillUnitMoved>()
            .add_systems(Update, zone_drain_skill_units);
        app
    }

    fn write_incoming(app: &mut App, body: Body) {
        app.world_mut()
            .resource_mut::<Messages<IncomingMessage>>()
            .write(IncomingMessage {
                sequence: 0,
                channel: GAMEPLAY,
                body,
            });
    }

    #[test]
    fn snapshot_and_spawn_preserve_variant_and_owner_type() {
        use net_contract::dto::SkillUnitOwnerType as Owner;

        let mut app = app_with_drain();
        let group = |unit_variant, owner_type| net::SkillUnitGroupState {
            skill_id: 521,
            unit_variant,
            owner_type,
            ..Default::default()
        };
        write_incoming(
            &mut app,
            Body::SkillUnitSnapshot(net::SkillUnitSnapshot {
                server_tick: 100,
                groups: vec![group(3, net::SkillUnitOwnerType::Mob as i32)],
            }),
        );
        write_incoming(
            &mut app,
            Body::SkillUnitSpawn(net::SkillUnitSpawn {
                group: Some(group(42, -1)),
            }),
        );
        app.update();

        let snapshot = app
            .world()
            .resource::<Messages<SkillUnitSnapshotReceived>>()
            .iter_current_update_messages()
            .next()
            .unwrap();
        assert_eq!(
            (
                snapshot.groups[0].unit_variant,
                snapshot.groups[0].owner_type
            ),
            (3, Owner::Mob)
        );
        let spawn = app
            .world()
            .resource::<Messages<SkillUnitSpawned>>()
            .iter_current_update_messages()
            .next()
            .unwrap();
        assert_eq!(
            (spawn.group.unit_variant, spawn.group.owner_type),
            (42, Owner::Unknown(-1))
        );
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

    #[test]
    fn move_drains_to_event() {
        let mut app = app_with_drain();
        write_incoming(
            &mut app,
            Body::SkillUnitMove(net::SkillUnitMove {
                group_id: 7,
                center_x: 51,
                center_y: 60,
                cells: vec![net::SkillUnitCellState {
                    cell_id: 700,
                    x: 50,
                    y: 59,
                    ..Default::default()
                }],
                server_tick: 9,
            }),
        );
        app.update();

        let events = app.world().resource::<Messages<SkillUnitMoved>>();
        let drained: Vec<_> = events.iter_current_update_messages().collect();
        assert_eq!(drained.len(), 1);
        assert_eq!((drained[0].center_x, drained[0].center_y), (51, 60));
        assert_eq!(drained[0].cells.len(), 1);
        assert_eq!(
            (
                drained[0].cells[0].cell_id,
                drained[0].cells[0].x,
                drained[0].cells[0].y
            ),
            (700, 50, 59)
        );
    }
}
