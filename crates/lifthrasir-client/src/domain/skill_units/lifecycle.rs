//! Server-driven HP updates and despawns for ground-skill units. The client
//! never times anything out itself; cells leave only via `SkillUnitDespawned`
//! (or a zone change reaping the `MapScoped` root).

use std::collections::HashSet;

use bevy::prelude::*;
use net_contract::events::{SkillUnitDespawned, SkillUnitMoved, SkillUnitUpdated};

use crate::domain::entities::registry::EntityRegistry;
use crate::domain::skill_units::components::{SkillUnitCell, SkillUnitGroup};
use crate::domain::skill_units::spawn::grid_coord;
use lifthrasir_runtime::rendering::coordinates::spawn_coords_to_world_position;

/// Apply server HP updates to a cell. An unknown group/cell (e.g. an update that
/// raced ahead of the spawn, or after despawn) is warned and ignored.
pub fn update_skill_units(
    mut events: MessageReader<SkillUnitUpdated>,
    mut cells: Query<&mut SkillUnitCell>,
) {
    for event in events.read() {
        let Some(mut cell) = cells
            .iter_mut()
            .find(|c| c.group_id == event.group_id && c.cell_id == event.cell_id)
        else {
            warn!(
                "SkillUnitUpdated for unknown group {} cell {}",
                event.group_id, event.cell_id
            );
            continue;
        };
        cell.hp = event.hp;
        cell.max_hp = event.max_hp;
    }
}

/// Recenter a group that moved with its caster (a pre-renewal performance field
/// following its performer). The root moves to the new center and each known
/// cell is re-offset under it, so group and cell visuals follow as children. The
/// root keeps its height: `Grounded` re-snaps it to the terrain every frame.
/// An unknown group (out of view, or already despawned) is warned and ignored.
pub fn move_skill_units(
    mut events: MessageReader<SkillUnitMoved>,
    mut groups: Query<(&SkillUnitGroup, &mut Transform), Without<SkillUnitCell>>,
    mut cells: Query<(&SkillUnitCell, &mut Transform), Without<SkillUnitGroup>>,
) {
    for event in events.read() {
        let (Some(cx), Some(cy)) = (grid_coord(event.center_x), grid_coord(event.center_y)) else {
            warn!(
                "SkillUnitMoved group {} has out-of-range center ({}, {})",
                event.group_id, event.center_x, event.center_y
            );
            continue;
        };
        let Some((_, mut root)) = groups
            .iter_mut()
            .find(|(g, _)| g.group_id == event.group_id)
        else {
            warn!("SkillUnitMoved for unknown group {}", event.group_id);
            continue;
        };
        let center = spawn_coords_to_world_position(cx, cy);
        root.translation.x = center.x;
        root.translation.z = center.z;

        for moved in &event.cells {
            let (Some(x), Some(y)) = (grid_coord(moved.x), grid_coord(moved.y)) else {
                continue;
            };
            let Some((_, mut transform)) = cells
                .iter_mut()
                .find(|(c, _)| c.group_id == event.group_id && c.cell_id == moved.cell_id)
            else {
                continue;
            };
            transform.translation = spawn_coords_to_world_position(x, y) - center;
        }
    }
}

/// Despawn the listed cells; when the group has no cells left, despawn the root
/// (recursively removing any remaining visuals). An unknown group is retried on
/// the next run, then warned and ignored.
pub fn despawn_skill_units(
    mut events: MessageReader<SkillUnitDespawned>,
    mut commands: Commands,
    mut entity_registry: ResMut<EntityRegistry>,
    groups: Query<(Entity, &SkillUnitGroup)>,
    cells: Query<(Entity, &SkillUnitCell)>,
    mut pending: Local<Vec<SkillUnitDespawned>>,
) {
    let retrying = std::mem::take(&mut *pending);
    for (event, is_retry) in retrying
        .iter()
        .map(|event| (event, true))
        .chain(events.read().map(|event| (event, false)))
    {
        let Some((root, _)) = groups.iter().find(|(_, g)| g.group_id == event.group_id) else {
            if is_retry {
                warn!("SkillUnitDespawned for unknown group {}", event.group_id);
            } else {
                pending.push(event.clone());
            }
            continue;
        };

        // Match against live cells so duplicate ids in one event cannot inflate
        // the count and despawn the root early; the root goes only when no live
        // cell remains outside this event's set.
        let removed: HashSet<u32> = event.cell_ids.iter().copied().collect();
        let mut remaining = 0;
        for (entity, cell) in cells.iter().filter(|(_, c)| c.group_id == event.group_id) {
            if removed.contains(&cell.cell_id) {
                if cell.flags.targetable {
                    entity_registry.unregister_entity_by_aid(cell.cell_id);
                }
                commands.entity(entity).despawn();
            } else {
                remaining += 1;
            }
        }

        if remaining == 0 {
            commands.entity(root).despawn();
        }
    }
}
