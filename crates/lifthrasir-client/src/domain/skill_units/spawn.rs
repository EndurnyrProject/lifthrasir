//! Group/cell spawning for server-authoritative ground-skill units.
//!
//! The server owns time and placement: cells spawn only in response to the
//! spawn/snapshot messages; there are no client-side lifetime timers. A group
//! lives until the server despawns its last cell (see `lifecycle`) or a zone
//! change reaps it via `MapScoped`.

use std::collections::HashMap;

use bevy::prelude::*;
use lifthrasir_data::GroundAnchor;
use net_contract::dto::{SkillUnitCellState, SkillUnitGroupState};
use net_contract::events::{SkillUnitSnapshotReceived, SkillUnitSpawned};

use crate::domain::entities::character::components::core::Grounded;
use crate::domain::entities::components::NetworkEntity;
use crate::domain::entities::picking::{on_sprite_click, on_sprite_out, on_sprite_over};
use crate::domain::entities::registry::EntityRegistry;
use crate::domain::entities::types::ObjectType;
use crate::domain::skill_units::components::{ScribbleMessage, SkillUnitCell, SkillUnitGroup};
use crate::domain::skill_units::visuals::spawn_effect_child;
use crate::domain::world::map_scoped::MapScoped;
use lifthrasir_assets::catalogs::effect::EffectCatalog;
use lifthrasir_runtime::rendering::coordinates::spawn_coords_to_world_position;

/// Half-extent of a targetable cell's click collider, matching the 5.0-unit
/// `RO_UNITS_PER_CELL` grid step (`utils::coordinates`) so one collider covers
/// exactly one cell.
const CELL_COLLIDER_HALF_SIZE: f32 = 2.5;

/// RO grid coordinates are non-negative; a negative wire value is malformed.
/// Reject it (the caller warns and skips) rather than wrapping it into a bogus
/// cell far off the map.
pub(super) fn grid_coord(value: i32) -> Option<u16> {
    u16::try_from(value).ok()
}

/// Spawn newly placed groups and zone-in snapshot groups through one code path.
///
/// Idempotent by `group_id`, including deferred spawns in the same frame.
/// Snapshots replace the complete visible set and run after incremental spawns;
/// the latest snapshot is authoritative when both arrive in one frame.
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
    if spawned.is_empty() && snapshots.is_empty() {
        return;
    }
    let catalog = catalog.as_deref();
    let mut live_groups: HashMap<_, _> = existing
        .iter()
        .map(|(root, group)| {
            (
                group.group_id,
                LiveGroup {
                    root,
                    targetable_cells: Vec::new(),
                },
            )
        })
        .collect();
    for cell in &existing_cells {
        if cell.flags.targetable
            && let Some(group) = live_groups.get_mut(&cell.group_id)
        {
            group.targetable_cells.push(cell.cell_id);
        }
    }
    for event in spawned.read() {
        spawn_group(
            &mut commands,
            &asset_server,
            &mut meshes,
            &mut materials,
            &mut entity_registry,
            catalog,
            &mut live_groups,
            &event.group,
        );
    }
    for snapshot in snapshots.read() {
        for (_, group) in live_groups.drain() {
            despawn_group(&mut commands, &mut entity_registry, &group);
        }
        for group in &snapshot.groups {
            spawn_group(
                &mut commands,
                &asset_server,
                &mut meshes,
                &mut materials,
                &mut entity_registry,
                catalog,
                &mut live_groups,
                group,
            );
        }
    }
}

struct LiveGroup {
    root: Entity,
    targetable_cells: Vec<u32>,
}

fn despawn_group(commands: &mut Commands, registry: &mut EntityRegistry, group: &LiveGroup) {
    for cell_id in &group.targetable_cells {
        registry.unregister_entity_by_aid(*cell_id);
    }
    commands.entity(group.root).despawn();
}

#[allow(clippy::too_many_arguments)]
fn spawn_group(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    entity_registry: &mut EntityRegistry,
    catalog: Option<&EffectCatalog>,
    live_groups: &mut HashMap<u64, LiveGroup>,
    group: &SkillUnitGroupState,
) {
    if let Some(old_group) = live_groups.remove(&group.group_id) {
        despawn_group(commands, entity_registry, &old_group);
    }

    let (Some(cx), Some(cy)) = (grid_coord(group.center_x), grid_coord(group.center_y)) else {
        warn!(
            "SkillUnit group {} has out-of-range center ({}, {}); skipping",
            group.group_id, group.center_x, group.center_y
        );
        return;
    };
    let center = spawn_coords_to_world_position(cx, cy);

    let root = commands
        .spawn((
            SkillUnitGroup {
                group_id: group.group_id,
                skill_id: group.skill_id,
                level: group.skill_level,
                owner_id: group.owner_id,
                phase: group.phase,
                created_tick: group.created_tick,
                expires_tick: group.expires_tick,
            },
            Transform::from_translation(center),
            Visibility::default(),
            // Snap the group to terrain height like any unit; cells inherit it.
            Grounded,
            MapScoped,
            Name::new(format!("SkillUnitGroup({})", group.group_id)),
        ))
        .id();

    let scribble = (group.skill_id == 220)
        .then(|| net_contract::dto::display_skill_text(&group.message))
        .filter(|message| !message.is_empty());
    let mut live_group = LiveGroup {
        root,
        targetable_cells: Vec::new(),
    };
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
        let cell_world = spawn_coords_to_world_position(cell_x, cell_y);
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
            live_group.targetable_cells.push(cell.cell_id);
            spawn_cell_collider(commands, meshes, cell_entity);
        }

        if let Some(message) = &scribble {
            commands
                .entity(cell_entity)
                .insert(ScribbleMessage(message.clone()));
        }
        cell_entities.push((cell_entity, cell));
    }
    live_groups.insert(group.group_id, live_group);

    // Skill-unit entities own persistent visuals only; `skill_unit` yields
    // none for a skill whose only entry is a one-shot cast.
    let Some(descriptor) = catalog.and_then(|c| c.skill_unit(group.skill_id)) else {
        if catalog.is_none_or(|c| c.skill(group.skill_id).is_none()) {
            warn!(
                "No effect catalog entry for skill-unit skill {}; group {} spawned without visuals",
                group.skill_id, group.group_id
            );
        }
        return;
    };

    match descriptor.ground_anchor {
        GroundAnchor::Group => {
            spawn_effect_child(
                commands,
                asset_server,
                meshes,
                materials,
                descriptor,
                group.phase,
                root,
            );
        }
        GroundAnchor::Cell | GroundAnchor::Footprint => {
            for (cell_entity, cell) in &cell_entities {
                if cell.flags.visible || descriptor.ground_anchor == GroundAnchor::Footprint {
                    spawn_effect_child(
                        commands,
                        asset_server,
                        meshes,
                        materials,
                        descriptor,
                        group.phase,
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
    // The plane's front face must point up (+Y) toward the camera or
    // bevy_picking backface-culls it and no click ever hits it.
    let mesh = meshes.add(Mesh::from(Plane3d::new(
        Vec3::Y,
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

#[cfg(test)]
mod scribble_tests {
    use super::*;
    use crate::domain::skill_units::components::ScribbleMessage;
    use net_contract::dto::{SkillUnitCellFlags, SkillUnitPhase};

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_resource::<EntityRegistry>()
            .add_message::<SkillUnitSpawned>()
            .add_message::<SkillUnitSnapshotReceived>()
            .add_systems(Update, spawn_skill_units);
        app
    }

    fn group(group_id: u64) -> SkillUnitGroupState {
        SkillUnitGroupState {
            group_id,
            skill_id: 220,
            skill_level: 1,
            owner_id: 42,
            center_x: 10,
            center_y: 20,
            phase: SkillUnitPhase::Active,
            created_tick: 0,
            expires_tick: 0,
            message: b"Hello\0\n\xff".to_vec(),
            cells: vec![SkillUnitCellState {
                cell_id: 7,
                x: 10,
                y: 20,
                hp: 0,
                max_hp: 0,
                flags: SkillUnitCellFlags::default(),
            }],
        }
    }

    #[test]
    fn scribble_text_belongs_to_cells_for_spawn_and_snapshot_without_a_caster() {
        let mut app = app();
        let mut group = group(1);
        app.world_mut().write_message(SkillUnitSpawned {
            group: group.clone(),
        });
        app.update();
        let world = app.world_mut();
        let (old_cell, text) = world
            .query::<(Entity, &ScribbleMessage)>()
            .single(world)
            .unwrap();
        assert_eq!(text.0, "Hello\u{fffd}");
        assert!(world.get::<SkillUnitCell>(old_cell).is_some());
        assert!(world.resource::<EntityRegistry>().get_entity(42).is_none());

        group.message = b"Replacement".to_vec();
        app.world_mut().write_message(SkillUnitSnapshotReceived {
            server_tick: 0,
            groups: vec![group.clone()],
        });
        app.update();
        let world = app.world_mut();
        assert!(world.get_entity(old_cell).is_err());
        let (_, text) = world
            .query::<(&SkillUnitCell, &ScribbleMessage)>()
            .single(world)
            .unwrap();
        assert_eq!(text.0, "Replacement");

        group.skill_id = 221;
        app.world_mut().write_message(SkillUnitSpawned {
            group: group.clone(),
        });
        app.update();
        let world = app.world_mut();
        assert_eq!(world.query::<&ScribbleMessage>().iter(world).count(), 0);

        group.skill_id = 220;
        group.message = b"\0\r\n".to_vec();
        app.world_mut().write_message(SkillUnitSpawned { group });
        app.update();
        let world = app.world_mut();
        assert_eq!(world.query::<&ScribbleMessage>().iter(world).count(), 0);
    }

    #[test]
    fn authoritative_snapshots_remove_absent_groups_and_replace_queued_spawns() {
        let mut app = app();
        let mut first = group(1);
        first.cells[0].flags.targetable = true;
        app.world_mut().write_message(SkillUnitSpawned {
            group: first.clone(),
        });
        app.update();
        let first_cell = app
            .world()
            .resource::<EntityRegistry>()
            .get_entity(7)
            .unwrap();

        let mut second = first.clone();
        second.group_id = 2;
        second.cells[0].cell_id = 8;
        second.message = b"Latest".to_vec();
        for _ in 0..2 {
            app.world_mut().write_message(SkillUnitSpawned {
                group: second.clone(),
            });
        }
        app.world_mut().write_message(SkillUnitSnapshotReceived {
            server_tick: 0,
            groups: vec![second],
        });
        app.update();
        let world = app.world_mut();
        assert!(world.get_entity(first_cell).is_err());
        assert!(world.resource::<EntityRegistry>().get_entity(7).is_none());
        assert_eq!(
            world
                .query::<&SkillUnitGroup>()
                .single(world)
                .unwrap()
                .group_id,
            2
        );
        let (cell, message) = world
            .query::<(&SkillUnitCell, &ScribbleMessage)>()
            .single(world)
            .unwrap();
        assert_eq!(cell.cell_id, 8);
        assert_eq!(message.0, "Latest");
        let second_cell = world.resource::<EntityRegistry>().get_entity(8).unwrap();
        assert!(world.get::<SkillUnitCell>(second_cell).is_some());

        app.world_mut().write_message(SkillUnitSnapshotReceived {
            server_tick: 0,
            groups: vec![first],
        });
        app.world_mut().write_message(SkillUnitSnapshotReceived {
            server_tick: 0,
            groups: vec![],
        });
        app.update();
        let world = app.world_mut();
        assert_eq!(world.query::<&SkillUnitGroup>().iter(world).count(), 0);
        assert_eq!(world.query::<&ScribbleMessage>().iter(world).count(), 0);
        assert!(world.get_entity(second_cell).is_err());
        assert!(world.resource::<EntityRegistry>().get_entity(7).is_none());
        assert!(world.resource::<EntityRegistry>().get_entity(8).is_none());
    }
}
