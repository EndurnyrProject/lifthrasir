//! Server-driven HP updates and despawns for ground-skill units. The client
//! never times anything out itself; cells leave only via `SkillUnitDespawned`
//! (or a zone change reaping the `MapScoped` root).

use std::collections::HashSet;

use bevy::prelude::*;
use net_contract::events::{SkillUnitDespawned, SkillUnitUpdated};

use super::components::{SkillUnitCell, SkillUnitGroup};
use crate::domain::entities::registry::EntityRegistry;

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

/// Despawn the listed cells; when the group has no cells left, despawn the root
/// (recursively removing any remaining visuals). An unknown group is warned and
/// ignored.
pub fn despawn_skill_units(
    mut events: MessageReader<SkillUnitDespawned>,
    mut commands: Commands,
    mut entity_registry: ResMut<EntityRegistry>,
    groups: Query<(Entity, &SkillUnitGroup)>,
    cells: Query<(Entity, &SkillUnitCell)>,
) {
    for event in events.read() {
        let Some((root, _)) = groups.iter().find(|(_, g)| g.group_id == event.group_id) else {
            warn!("SkillUnitDespawned for unknown group {}", event.group_id);
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
