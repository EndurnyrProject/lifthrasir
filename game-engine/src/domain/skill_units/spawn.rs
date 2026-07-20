//! Group/cell spawning for server-authoritative ground-skill units.
//!
//! The server owns time and placement: cells spawn only in response to the
//! spawn/snapshot messages; there are no client-side lifetime timers. A group
//! lives until the server despawns its last cell (see `lifecycle`) or a zone
//! change reaps it via `MapScoped`.

use bevy::prelude::*;
use lifthrasir_data::GroundAnchor;
use net_contract::dto::{SkillUnitCellState, SkillUnitGroupState};
use net_contract::events::{SkillUnitSnapshotReceived, SkillUnitSpawned};

use super::components::{SkillUnitCell, SkillUnitGroup};
use super::visuals::spawn_effect_child;
use crate::domain::entities::character::components::core::Grounded;
use crate::domain::entities::components::NetworkEntity;
use crate::domain::entities::picking::{on_sprite_click, on_sprite_out, on_sprite_over};
use crate::domain::entities::registry::EntityRegistry;
use crate::domain::entities::types::ObjectType;
use crate::domain::world::map_scoped::MapScoped;
use crate::infrastructure::effect::EffectCatalog;
use crate::utils::coordinates::spawn_coords_to_world_position;

/// Half-extent of a targetable cell's click collider, matching the 5.0-unit
/// `RO_UNITS_PER_CELL` grid step (`utils::coordinates`) so one collider covers
/// exactly one cell.
const CELL_COLLIDER_HALF_SIZE: f32 = 2.5;

/// RO grid coordinates are non-negative; a negative wire value is malformed.
/// Reject it (the caller warns and skips) rather than wrapping it into a bogus
/// cell far off the map.
fn grid_coord(value: i32) -> Option<u16> {
    u16::try_from(value).ok()
}

/// Spawn newly placed groups and zone-in snapshot groups through one code path.
///
/// Idempotent by `group_id`: a live group re-entering view (or a duplicate
/// spawn) despawns the existing root first, so visuals replace instead of stack.
/// The `existing` query is read at system start, so two references to the same
/// `group_id` within a single frame can both spawn; the server does not do that
/// (a group arrives via spawn OR snapshot, not both at once).
#[allow(clippy::too_many_arguments)]
pub fn spawn_skill_units(
    mut spawned: MessageReader<SkillUnitSpawned>,
    mut snapshots: MessageReader<SkillUnitSnapshotReceived>,
    mut commands: Commands,
    catalog: Option<Res<EffectCatalog>>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut entity_registry: ResMut<EntityRegistry>,
    existing: Query<(Entity, &SkillUnitGroup)>,
    existing_cells: Query<&SkillUnitCell>,
) {
    let catalog = catalog.as_deref();
    for event in spawned.read() {
        spawn_group(
            &mut commands,
            &asset_server,
            &mut meshes,
            &mut materials,
            &mut entity_registry,
            catalog,
            &existing,
            &existing_cells,
            &event.group,
        );
    }
    for snapshot in snapshots.read() {
        for group in &snapshot.groups {
            spawn_group(
                &mut commands,
                &asset_server,
                &mut meshes,
                &mut materials,
                &mut entity_registry,
                catalog,
                &existing,
                &existing_cells,
                group,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_group(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    entity_registry: &mut EntityRegistry,
    catalog: Option<&EffectCatalog>,
    existing: &Query<(Entity, &SkillUnitGroup)>,
    existing_cells: &Query<&SkillUnitCell>,
    group: &SkillUnitGroupState,
) {
    if let Some((old_root, _)) = existing.iter().find(|(_, g)| g.group_id == group.group_id) {
        for cell in existing_cells
            .iter()
            .filter(|c| c.group_id == group.group_id && c.flags.targetable)
        {
            entity_registry.unregister_entity_by_aid(cell.cell_id);
        }
        commands.entity(old_root).despawn();
    }

    let (Some(cx), Some(cy)) = (grid_coord(group.center_x), grid_coord(group.center_y)) else {
        warn!(
            "SkillUnit group {} has out-of-range center ({}, {}); skipping",
            group.group_id, group.center_x, group.center_y
        );
        return;
    };
    let center = spawn_coords_to_world_position(cx, cy, 0, 0);

    let root = commands
        .spawn((
            SkillUnitGroup {
                group_id: group.group_id,
                skill_id: group.skill_id,
                level: group.skill_level,
                owner_id: group.owner_id,
            },
            Transform::from_translation(center),
            Visibility::default(),
            // Snap the group to terrain height like any unit; cells inherit it.
            Grounded,
            MapScoped,
            Name::new(format!("SkillUnitGroup({})", group.group_id)),
        ))
        .id();

    let mut cell_entities: Vec<(Entity, &SkillUnitCellState)> =
        Vec::with_capacity(group.cells.len());
    for cell in &group.cells {
        let (Some(cell_x), Some(cell_y)) = (grid_coord(cell.x), grid_coord(cell.y)) else {
            warn!(
                "SkillUnit group {} cell {} has out-of-range coords ({}, {}); skipping",
                group.group_id, cell.cell_id, cell.x, cell.y
            );
            continue;
        };
        let cell_world = spawn_coords_to_world_position(cell_x, cell_y, 0, 0);
        let cell_entity = commands
            .spawn((
                SkillUnitCell {
                    group_id: group.group_id,
                    cell_id: cell.cell_id,
                    flags: cell.flags,
                    hp: cell.hp,
                    max_hp: cell.max_hp,
                },
                Transform::from_translation(cell_world - center),
                Visibility::default(),
                ChildOf(root),
            ))
            .id();

        if cell.flags.targetable {
            commands.entity(cell_entity).insert(NetworkEntity::new(
                cell.cell_id,
                cell.cell_id,
                ObjectType::SkillUnit,
            ));
            entity_registry.register_entity(cell.cell_id, cell_entity);
            spawn_cell_collider(commands, meshes, cell_entity);
        }

        cell_entities.push((cell_entity, cell));
    }

    let Some(descriptor) = catalog.and_then(|c| c.get(group.skill_id)) else {
        warn!(
            "No effect catalog entry for skill-unit skill {}; group {} spawned without visuals",
            group.skill_id, group.group_id
        );
        return;
    };

    // Skill-unit entities own persistent visuals only. Some ground skills use
    // a short-lived administrative group for scheduling; their one-shot visual
    // belongs to the damage/ground trigger instead of this group center.
    if !descriptor.repeating {
        return;
    }

    match descriptor.ground_anchor {
        GroundAnchor::Group => {
            spawn_effect_child(commands, asset_server, meshes, materials, descriptor, root);
        }
        GroundAnchor::Cell => {
            for (cell_entity, cell) in &cell_entities {
                if cell.flags.visible {
                    spawn_effect_child(
                        commands,
                        asset_server,
                        meshes,
                        materials,
                        descriptor,
                        *cell_entity,
                    );
                }
            }
        }
    }
}

/// Spawn a flat, invisible (unmaterialed) click collider as a child of a
/// targetable cell. The STR effect is the visible thing; this quad exists only
/// so `bevy_picking`'s mesh backend has geometry to raycast against, mirroring
/// how a mob/NPC's body billboard carries `Pickable` while its `NetworkEntity`
/// lives one level up on the root (here: the cell).
fn spawn_cell_collider(commands: &mut Commands, meshes: &mut Assets<Mesh>, cell_entity: Entity) {
    // World up is -Y here; the plane's front face must point along NEG_Y or
    // bevy_picking backface-culls it and no click ever hits it.
    let mesh = meshes.add(Mesh::from(Plane3d::new(
        Vec3::NEG_Y,
        Vec2::splat(CELL_COLLIDER_HALF_SIZE),
    )));

    commands
        .spawn((
            Mesh3d(mesh),
            Transform::default(),
            GlobalTransform::default(),
            Visibility::default(),
            InheritedVisibility::default(),
            ViewVisibility::default(),
            Pickable::default(),
            ChildOf(cell_entity),
        ))
        .observe(on_sprite_over)
        .observe(on_sprite_out)
        .observe(on_sprite_click);
}
