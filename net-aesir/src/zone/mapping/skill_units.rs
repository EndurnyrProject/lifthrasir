use crate::proto::aesir::net;
use net_contract::dto::{
    SkillUnitCellFlags, SkillUnitCellState, SkillUnitDespawnReason, SkillUnitGroupState,
    SkillUnitUpdateReason,
};
use net_contract::events::{
    SkillUnitDespawned, SkillUnitSnapshotReceived, SkillUnitSpawned, SkillUnitUpdated,
};

fn skill_unit_cell_flags(flags: u32) -> SkillUnitCellFlags {
    SkillUnitCellFlags {
        targetable: flags & 0b0000_0001 != 0,
        blocks_movement: flags & 0b0000_0010 != 0,
        blocks_projectiles: flags & 0b0000_0100 != 0,
        consumable_water: flags & 0b0000_1000 != 0,
        visible: flags & 0b0001_0000 != 0,
    }
}

fn skill_unit_cell(c: net::SkillUnitCellState) -> SkillUnitCellState {
    SkillUnitCellState {
        cell_id: c.cell_id,
        x: c.x,
        y: c.y,
        hp: c.hp,
        max_hp: c.max_hp,
        flags: skill_unit_cell_flags(c.flags),
    }
}

fn skill_unit_group(g: net::SkillUnitGroupState) -> SkillUnitGroupState {
    SkillUnitGroupState {
        group_id: g.group_id,
        skill_id: g.skill_id,
        skill_level: g.skill_level,
        owner_id: g.owner_id,
        center_x: g.center_x,
        center_y: g.center_y,
        cells: g.cells.into_iter().map(skill_unit_cell).collect(),
    }
}

fn skill_unit_update_reason(value: i32) -> SkillUnitUpdateReason {
    match net::SkillUnitUpdateReason::try_from(value) {
        Ok(net::SkillUnitUpdateReason::Unspecified) | Err(_) => SkillUnitUpdateReason::Unspecified,
        Ok(net::SkillUnitUpdateReason::Damage) => SkillUnitUpdateReason::Damage,
        Ok(net::SkillUnitUpdateReason::Decay) => SkillUnitUpdateReason::Decay,
    }
}

fn skill_unit_despawn_reason(value: i32) -> SkillUnitDespawnReason {
    match net::SkillUnitDespawnReason::try_from(value) {
        Ok(net::SkillUnitDespawnReason::Unspecified) | Err(_) => {
            SkillUnitDespawnReason::Unspecified
        }
        Ok(net::SkillUnitDespawnReason::Expired) => SkillUnitDespawnReason::Expired,
        Ok(net::SkillUnitDespawnReason::Destroyed) => SkillUnitDespawnReason::Destroyed,
        Ok(net::SkillUnitDespawnReason::SourceConsumed) => SkillUnitDespawnReason::SourceConsumed,
        Ok(net::SkillUnitDespawnReason::Lifecycle) => SkillUnitDespawnReason::Lifecycle,
        Ok(net::SkillUnitDespawnReason::MapShutdown) => SkillUnitDespawnReason::MapShutdown,
        Ok(net::SkillUnitDespawnReason::LeftView) => SkillUnitDespawnReason::LeftView,
        Ok(net::SkillUnitDespawnReason::Canceled) => SkillUnitDespawnReason::Canceled,
    }
}

pub fn skill_unit_snapshot(s: net::SkillUnitSnapshot) -> SkillUnitSnapshotReceived {
    SkillUnitSnapshotReceived {
        server_tick: s.server_tick,
        groups: s.groups.into_iter().map(skill_unit_group).collect(),
    }
}

/// `None` when the packet carries no group; the caller decides how to log that.
pub fn skill_unit_spawn(s: net::SkillUnitSpawn) -> Option<SkillUnitSpawned> {
    s.group
        .map(skill_unit_group)
        .map(|group| SkillUnitSpawned { group })
}

pub fn skill_unit_update(u: net::SkillUnitUpdate) -> SkillUnitUpdated {
    SkillUnitUpdated {
        group_id: u.group_id,
        cell_id: u.cell_id,
        hp: u.hp,
        max_hp: u.max_hp,
        hp_delta: u.hp_delta,
        reason: skill_unit_update_reason(u.reason),
    }
}

pub fn skill_unit_despawn(d: net::SkillUnitDespawn) -> SkillUnitDespawned {
    SkillUnitDespawned {
        group_id: d.group_id,
        cell_ids: d.cell_ids,
        reason: skill_unit_despawn_reason(d.reason),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_flags_decode_each_bit_independently() {
        assert_eq!(
            skill_unit_cell_flags(0b0000_0001),
            SkillUnitCellFlags {
                targetable: true,
                ..Default::default()
            }
        );
        assert_eq!(
            skill_unit_cell_flags(0b0000_0010),
            SkillUnitCellFlags {
                blocks_movement: true,
                ..Default::default()
            }
        );
        assert_eq!(
            skill_unit_cell_flags(0b0000_0100),
            SkillUnitCellFlags {
                blocks_projectiles: true,
                ..Default::default()
            }
        );
        assert_eq!(
            skill_unit_cell_flags(0b0000_1000),
            SkillUnitCellFlags {
                consumable_water: true,
                ..Default::default()
            }
        );
        assert_eq!(
            skill_unit_cell_flags(0b0001_0000),
            SkillUnitCellFlags {
                visible: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn cell_flags_decode_combined_bits() {
        let flags = skill_unit_cell_flags(0b0001_0101);
        assert!(flags.targetable);
        assert!(!flags.blocks_movement);
        assert!(flags.blocks_projectiles);
        assert!(!flags.consumable_water);
        assert!(flags.visible);
    }

    #[test]
    fn cell_flags_zero_is_all_false() {
        assert_eq!(skill_unit_cell_flags(0), SkillUnitCellFlags::default());
    }

    fn sample_cell(cell_id: u32, flags: u32) -> net::SkillUnitCellState {
        net::SkillUnitCellState {
            cell_id,
            x: 150,
            y: 150,
            hp: 100,
            max_hp: 100,
            flags,
        }
    }

    fn sample_group(
        group_id: u64,
        cells: Vec<net::SkillUnitCellState>,
    ) -> net::SkillUnitGroupState {
        net::SkillUnitGroupState {
            group_id,
            skill_id: 89,
            skill_level: 10,
            owner_type: net::SkillUnitOwnerType::Player as i32,
            owner_id: 42,
            center_x: 150,
            center_y: 150,
            created_tick: 1000,
            expires_tick: 9000,
            cells,
        }
    }

    #[test]
    fn snapshot_maps_n_groups() {
        let snapshot = skill_unit_snapshot(net::SkillUnitSnapshot {
            server_tick: 12345,
            groups: vec![
                sample_group(1, vec![sample_cell(100, 0b0000_0001)]),
                sample_group(2, vec![sample_cell(200, 0), sample_cell(201, 0)]),
                sample_group(3, vec![]),
            ],
        });

        assert_eq!(snapshot.server_tick, 12345);
        assert_eq!(snapshot.groups.len(), 3);
        assert_eq!(snapshot.groups[0].group_id, 1);
        assert_eq!(snapshot.groups[0].cells.len(), 1);
        assert!(snapshot.groups[0].cells[0].flags.targetable);
        assert_eq!(snapshot.groups[1].group_id, 2);
        assert_eq!(snapshot.groups[1].cells.len(), 2);
        assert_eq!(snapshot.groups[2].group_id, 3);
        assert!(snapshot.groups[2].cells.is_empty());
    }

    #[test]
    fn group_state_drops_owner_type_and_tick_fields() {
        let mapped = skill_unit_group(sample_group(1, vec![]));

        assert_eq!(mapped.group_id, 1);
        assert_eq!(mapped.skill_id, 89);
        assert_eq!(mapped.skill_level, 10);
        assert_eq!(mapped.owner_id, 42);
        assert_eq!(mapped.center_x, 150);
        assert_eq!(mapped.center_y, 150);
    }

    #[test]
    fn spawn_maps_present_group() {
        let spawned = skill_unit_spawn(net::SkillUnitSpawn {
            group: Some(sample_group(7, vec![sample_cell(700, 0b0001_0000)])),
        })
        .expect("group should map");

        assert_eq!(spawned.group.group_id, 7);
        assert_eq!(spawned.group.cells.len(), 1);
        assert!(spawned.group.cells[0].flags.visible);
    }

    #[test]
    fn spawn_returns_none_without_group() {
        assert!(skill_unit_spawn(net::SkillUnitSpawn { group: None }).is_none());
    }

    #[test]
    fn update_maps_hp_and_damage_reason() {
        let updated = skill_unit_update(net::SkillUnitUpdate {
            group_id: 7,
            cell_id: 700,
            hp: 50,
            max_hp: 100,
            hp_delta: -50,
            source_type: net::SkillUnitOwnerType::Player as i32,
            source_id: 150001,
            reason: net::SkillUnitUpdateReason::Damage as i32,
            server_tick: 5000,
        });

        assert_eq!(updated.group_id, 7);
        assert_eq!(updated.cell_id, 700);
        assert_eq!(updated.hp, 50);
        assert_eq!(updated.max_hp, 100);
        assert_eq!(updated.hp_delta, -50);
        assert_eq!(updated.reason, SkillUnitUpdateReason::Damage);
    }

    #[test]
    fn update_maps_decay_reason() {
        let updated = skill_unit_update(net::SkillUnitUpdate {
            reason: net::SkillUnitUpdateReason::Decay as i32,
            ..Default::default()
        });

        assert_eq!(updated.reason, SkillUnitUpdateReason::Decay);
    }

    #[test]
    fn update_unknown_reason_defaults_to_unspecified() {
        let updated = skill_unit_update(net::SkillUnitUpdate {
            reason: 99,
            ..Default::default()
        });

        assert_eq!(updated.reason, SkillUnitUpdateReason::Unspecified);
    }

    #[test]
    fn despawn_maps_cell_ids_and_every_reason() {
        let cases = [
            (
                net::SkillUnitDespawnReason::Unspecified,
                SkillUnitDespawnReason::Unspecified,
            ),
            (
                net::SkillUnitDespawnReason::Expired,
                SkillUnitDespawnReason::Expired,
            ),
            (
                net::SkillUnitDespawnReason::Destroyed,
                SkillUnitDespawnReason::Destroyed,
            ),
            (
                net::SkillUnitDespawnReason::SourceConsumed,
                SkillUnitDespawnReason::SourceConsumed,
            ),
            (
                net::SkillUnitDespawnReason::Lifecycle,
                SkillUnitDespawnReason::Lifecycle,
            ),
            (
                net::SkillUnitDespawnReason::MapShutdown,
                SkillUnitDespawnReason::MapShutdown,
            ),
            (
                net::SkillUnitDespawnReason::LeftView,
                SkillUnitDespawnReason::LeftView,
            ),
            (
                net::SkillUnitDespawnReason::Canceled,
                SkillUnitDespawnReason::Canceled,
            ),
        ];

        for (wire, expected) in cases {
            let despawned = skill_unit_despawn(net::SkillUnitDespawn {
                group_id: 7,
                cell_ids: vec![700, 701],
                reason: wire as i32,
                server_tick: 5000,
            });

            assert_eq!(despawned.group_id, 7);
            assert_eq!(despawned.cell_ids, vec![700, 701]);
            assert_eq!(despawned.reason, expected);
        }
    }

    #[test]
    fn despawn_unknown_reason_defaults_to_unspecified() {
        let despawned = skill_unit_despawn(net::SkillUnitDespawn {
            group_id: 7,
            cell_ids: vec![],
            reason: 99,
            server_tick: 0,
        });

        assert_eq!(despawned.reason, SkillUnitDespawnReason::Unspecified);
    }
}
