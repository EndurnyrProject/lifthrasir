//! Group/cell lifecycle for server-authoritative ground-skill units.
//!
//! The server owns time and placement: cells spawn, update, and despawn only in
//! response to the four skill-unit messages. There are no client-side lifetime
//! timers anywhere here — a group lives until the server despawns its last cell
//! (or a zone change reaps it via `MapScoped`).
//!
//! Persistent visuals are attached as effect children of the group root (for a
//! `GroundAnchor::Group` skill like Storm Gust) or of each visible cell (for a
//! `GroundAnchor::Cell` skill like Ice Wall). They are children rather than
//! entity-anchored effects because `EffectAnchor::Entity` only follows the
//! anchor's transform and does not tear down when the anchor dies; parenting
//! makes the recursive group despawn remove every visual, which is the invariant.

use std::collections::HashSet;

use bevy::prelude::*;
use lifthrasir_data::{EffectDescriptor, GroundAnchor};
use net_contract::dto::{SkillUnitCellState, SkillUnitGroupState};
use net_contract::events::{
    SkillUnitDespawned, SkillUnitSnapshotReceived, SkillUnitSpawned, SkillUnitUpdated,
};

use super::components::{SkillUnitCell, SkillUnitGroup};
use crate::domain::effects::components::EffectAnchor;
use crate::domain::effects::spawn_effect;
use crate::domain::effects::triggers::{descriptor_tint, load_effect};
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

/// Footprint and height of the persistent placeholder prism spawned for a
/// ground descriptor that has a `vfx` key but no STR (e.g. Ice Wall). Roughly
/// one grid cell wide, standing above the ground along world-up (`-Y`).
const VFX_PRISM_FOOTPRINT: f32 = 3.5;
const VFX_PRISM_HEIGHT: f32 = 6.0;
/// Translucency of the placeholder prism so it reads as a rough crystal, not a
/// solid block.
const VFX_PRISM_ALPHA: f32 = 0.5;

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

/// Spawn the descriptor's persistent visual as a child of `parent` (root or
/// cell), at the parent's origin. Preference order:
///   1. an STR effect when the descriptor has one;
///   2. otherwise, for a `vfx`-only descriptor (e.g. Ice Wall, which the classic
///      client hardcodes and no STR exists for), a simple translucent placeholder
///      prism tinted by the descriptor — `PlayProceduralVfx` is fire-and-forget
///      and cannot back a persistent wall cell, so the wall lives here as a child
///      that despawns with the group/cell.
///
/// A descriptor with neither spawns nothing; the group is still
/// gameplay-relevant, so that is not an error.
fn spawn_effect_child(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    descriptor: &EffectDescriptor,
    parent: Entity,
) {
    if let Some(handle) = load_effect(asset_server, descriptor) {
        let effect = spawn_effect(
            commands,
            handle,
            EffectAnchor::Position(Vec3::ZERO),
            descriptor.repeating,
            descriptor_tint(descriptor),
            None,
        );
        commands.entity(effect).insert(ChildOf(parent));
        return;
    }

    if descriptor.vfx.is_some() {
        spawn_vfx_placeholder(commands, meshes, materials, descriptor, parent);
    }
}

/// Spawn a persistent translucent prism placeholder for a `vfx`-only ground
/// descriptor. Unlit, alpha-blended, tinted by the descriptor color; lifted along
/// world-up (`-Y`) so it stands on the cell rather than sinking through it. Dead
/// simple by design — no looping hanabi, no animation; child hierarchy handles
/// teardown when the group/cell despawns.
fn spawn_vfx_placeholder(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    descriptor: &EffectDescriptor,
    parent: Entity,
) {
    let mesh = meshes.add(Mesh::from(Cuboid::new(
        VFX_PRISM_FOOTPRINT,
        VFX_PRISM_HEIGHT,
        VFX_PRISM_FOOTPRINT,
    )));
    let tint = descriptor_tint(descriptor).with_alpha(VFX_PRISM_ALPHA);
    let material = materials.add(StandardMaterial {
        base_color: tint,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_xyz(0.0, -VFX_PRISM_HEIGHT / 2.0, 0.0),
        Visibility::default(),
        ChildOf(parent),
    ));
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::effects::components::ActiveEffect;
    use crate::domain::entities::registry::EntityRegistry;
    use crate::infrastructure::effect::{EffectCatalog, EffectDataAsset, LoadedEffectAsset};
    use lifthrasir_data::{EffectDescriptor, EffectPlacement};
    use net_contract::dto::{SkillUnitCellFlags, SkillUnitCellState, SkillUnitGroupState};
    use std::collections::BTreeMap;

    fn targetable_cell(cell_id: u32, x: i32, y: i32, visible: bool) -> SkillUnitCellState {
        SkillUnitCellState {
            cell_id,
            x,
            y,
            hp: 100,
            max_hp: 100,
            flags: SkillUnitCellFlags {
                targetable: true,
                visible,
                ..Default::default()
            },
        }
    }

    const STORM_GUST: u32 = 89; // seeded Ground/Group anchor with an STR.

    fn seeded_catalog() -> EffectCatalog {
        let ron = include_str!("../../../../assets/data/ron/effects.ron");
        let asset = ron::from_str::<EffectDataAsset>(ron).expect("seed RON");
        EffectCatalog::from_skill_effect_data(asset.0.skills)
    }

    fn cell_anchored_catalog(skill_id: u32) -> EffectCatalog {
        let mut skills = BTreeMap::new();
        skills.insert(
            skill_id,
            EffectDescriptor {
                str: Some("icewall.str".into()),
                vfx: None,
                sound: None,
                placement: EffectPlacement::Ground,
                color: [1.0, 1.0, 1.0, 1.0],
                repeating: true,
                ground_anchor: GroundAnchor::Cell,
            },
        );
        EffectCatalog::from_skill_effect_data(skills)
    }

    /// Cell-anchored descriptor with a `vfx` key and NO STR (the Ice Wall shape):
    /// spawns a persistent placeholder prism per visible cell instead of an STR.
    fn cell_anchored_vfx_catalog(skill_id: u32) -> EffectCatalog {
        let mut skills = BTreeMap::new();
        skills.insert(
            skill_id,
            EffectDescriptor {
                str: None,
                vfx: Some("ice_wall".into()),
                sound: None,
                placement: EffectPlacement::Ground,
                color: [1.0, 1.0, 1.0, 1.0],
                repeating: true,
                ground_anchor: GroundAnchor::Cell,
            },
        );
        EffectCatalog::from_skill_effect_data(skills)
    }

    fn test_app(catalog: EffectCatalog) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<LoadedEffectAsset>()
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_resource::<EntityRegistry>()
            .add_message::<SkillUnitSpawned>()
            .add_message::<SkillUnitSnapshotReceived>()
            .add_message::<SkillUnitUpdated>()
            .add_message::<SkillUnitDespawned>()
            .insert_resource(catalog)
            .add_systems(
                Update,
                (spawn_skill_units, update_skill_units, despawn_skill_units),
            );
        app
    }

    fn cell(cell_id: u32, x: i32, y: i32, visible: bool) -> SkillUnitCellState {
        SkillUnitCellState {
            cell_id,
            x,
            y,
            hp: 100,
            max_hp: 100,
            flags: SkillUnitCellFlags {
                visible,
                ..Default::default()
            },
        }
    }

    fn group(group_id: u64, skill_id: u32, cells: Vec<SkillUnitCellState>) -> SkillUnitGroupState {
        SkillUnitGroupState {
            group_id,
            skill_id,
            skill_level: 10,
            owner_id: 42,
            center_x: 40,
            center_y: 50,
            cells,
        }
    }

    fn roots(app: &mut App) -> usize {
        app.world_mut()
            .query::<&SkillUnitGroup>()
            .iter(app.world())
            .count()
    }

    fn cell_count(app: &mut App) -> usize {
        app.world_mut()
            .query::<&SkillUnitCell>()
            .iter(app.world())
            .count()
    }

    fn effects(app: &mut App) -> usize {
        app.world_mut()
            .query::<&ActiveEffect>()
            .iter(app.world())
            .count()
    }

    /// Placeholder prisms carry a `StandardMaterial`; the click colliders on
    /// targetable cells are material-less, so this counts only the vfx prisms.
    fn placeholders(app: &mut App) -> usize {
        app.world_mut()
            .query::<&MeshMaterial3d<StandardMaterial>>()
            .iter(app.world())
            .count()
    }

    #[test]
    fn spawn_creates_root_and_cells_at_world_positions() {
        let mut app = test_app(seeded_catalog());
        app.world_mut().write_message(SkillUnitSpawned {
            group: group(
                1,
                STORM_GUST,
                vec![cell(100, 40, 50, true), cell(101, 41, 50, true)],
            ),
        });
        app.update();

        assert_eq!(roots(&mut app), 1, "one group root");
        assert_eq!(cell_count(&mut app), 2, "two cells");

        // Root sits at the group center; each cell is a child positioned so its
        // world transform (root + local) lands on the cell's world coordinate.
        let center = spawn_coords_to_world_position(40, 50, 0, 0);
        let mut root_query = app.world_mut().query::<(Entity, &Transform)>();
        let root = root_query
            .iter(app.world())
            .find(|(e, _)| app.world().get::<SkillUnitGroup>(*e).is_some())
            .map(|(e, t)| (e, *t))
            .expect("root");
        assert_eq!(root.1.translation, center);

        let mut cells = app
            .world_mut()
            .query::<(&SkillUnitCell, &Transform, &ChildOf)>();
        let mut seen = 0;
        for (cell, transform, child_of) in cells.iter(app.world()) {
            assert_eq!(child_of.parent(), root.0, "cell is a child of the root");
            let world = root.1.translation + transform.translation;
            let cell_x = if cell.cell_id == 100 { 40 } else { 41 };
            let want = spawn_coords_to_world_position(cell_x, 50, 0, 0);
            assert_eq!(world, want, "cell {} world position", cell.cell_id);
            seen += 1;
        }
        assert_eq!(seen, 2);
    }

    #[test]
    fn snapshot_bulk_spawns_all_groups() {
        let mut app = test_app(seeded_catalog());
        app.world_mut().write_message(SkillUnitSnapshotReceived {
            server_tick: 7,
            groups: vec![
                group(1, STORM_GUST, vec![cell(100, 40, 50, true)]),
                group(
                    2,
                    STORM_GUST,
                    vec![cell(200, 41, 50, true), cell(201, 42, 50, true)],
                ),
            ],
        });
        app.update();

        assert_eq!(roots(&mut app), 2, "both snapshot groups spawn");
        assert_eq!(cell_count(&mut app), 3, "all snapshot cells spawn");
    }

    #[test]
    fn duplicate_spawn_replaces_and_does_not_stack() {
        let mut app = test_app(seeded_catalog());
        let cells = vec![cell(100, 40, 50, true), cell(101, 41, 50, true)];

        app.world_mut().write_message(SkillUnitSpawned {
            group: group(1, STORM_GUST, cells.clone()),
        });
        app.update();
        assert_eq!(roots(&mut app), 1);
        assert_eq!(cell_count(&mut app), 2);
        assert_eq!(effects(&mut app), 1, "one group-anchored effect");

        app.world_mut().write_message(SkillUnitSpawned {
            group: group(1, STORM_GUST, cells),
        });
        app.update();

        assert_eq!(roots(&mut app), 1, "duplicate replaces the root");
        assert_eq!(cell_count(&mut app), 2, "cells did not stack");
        assert_eq!(effects(&mut app), 1, "visual replaced, not stacked");
    }

    #[test]
    fn subset_despawn_keeps_root_last_cell_removes_root() {
        let mut app = test_app(seeded_catalog());
        app.world_mut().write_message(SkillUnitSpawned {
            group: group(
                1,
                STORM_GUST,
                vec![cell(100, 40, 50, true), cell(101, 41, 50, true)],
            ),
        });
        app.update();
        assert_eq!(cell_count(&mut app), 2);

        // Subset despawn: one cell goes, the root stays.
        app.world_mut().write_message(SkillUnitDespawned {
            group_id: 1,
            cell_ids: vec![100],
            reason: Default::default(),
        });
        app.update();
        assert_eq!(roots(&mut app), 1, "root survives a subset despawn");
        assert_eq!(cell_count(&mut app), 1, "one cell removed");

        // Last cell goes: the root despawns with it.
        app.world_mut().write_message(SkillUnitDespawned {
            group_id: 1,
            cell_ids: vec![101],
            reason: Default::default(),
        });
        app.update();
        assert_eq!(cell_count(&mut app), 0, "no cells left");
        assert_eq!(
            roots(&mut app),
            0,
            "root despawns once the last cell is gone"
        );
    }

    #[test]
    fn duplicate_cell_ids_in_one_despawn_do_not_remove_root_early() {
        let mut app = test_app(seeded_catalog());
        app.world_mut().write_message(SkillUnitSpawned {
            group: group(
                1,
                STORM_GUST,
                vec![cell(100, 40, 50, true), cell(101, 41, 50, true)],
            ),
        });
        app.update();

        // Duplicated id for a single cell must count as one removal, not two.
        app.world_mut().write_message(SkillUnitDespawned {
            group_id: 1,
            cell_ids: vec![100, 100],
            reason: Default::default(),
        });
        app.update();

        assert_eq!(
            cell_count(&mut app),
            1,
            "only the one referenced cell is gone"
        );
        assert_eq!(roots(&mut app), 1, "root survives; a live cell remains");
    }

    #[test]
    fn out_of_range_center_skips_the_group() {
        let mut app = test_app(seeded_catalog());
        let mut g = group(1, STORM_GUST, vec![cell(100, 40, 50, true)]);
        g.center_x = -1;
        app.world_mut().write_message(SkillUnitSpawned { group: g });
        app.update();

        assert_eq!(
            roots(&mut app),
            0,
            "malformed center is rejected, not wrapped"
        );
        assert_eq!(cell_count(&mut app), 0);
    }

    #[test]
    fn out_of_range_cell_skips_only_that_cell() {
        let mut app = test_app(seeded_catalog());
        app.world_mut().write_message(SkillUnitSpawned {
            group: group(
                1,
                STORM_GUST,
                vec![cell(100, 40, 50, true), cell(101, -5, 50, true)],
            ),
        });
        app.update();

        assert_eq!(roots(&mut app), 1, "the group still spawns");
        assert_eq!(cell_count(&mut app), 1, "the out-of-range cell is dropped");
    }

    #[test]
    fn unknown_group_on_update_is_a_noop() {
        let mut app = test_app(seeded_catalog());
        app.world_mut().write_message(SkillUnitUpdated {
            group_id: 999,
            cell_id: 1,
            hp: 10,
            max_hp: 100,
            hp_delta: -90,
            reason: Default::default(),
        });
        app.update();
        assert_eq!(roots(&mut app), 0, "no panic, nothing spawned");
    }

    #[test]
    fn update_mutates_known_cell_hp() {
        let mut app = test_app(seeded_catalog());
        app.world_mut().write_message(SkillUnitSpawned {
            group: group(1, STORM_GUST, vec![cell(100, 40, 50, true)]),
        });
        app.update();

        app.world_mut().write_message(SkillUnitUpdated {
            group_id: 1,
            cell_id: 100,
            hp: 25,
            max_hp: 100,
            hp_delta: -75,
            reason: Default::default(),
        });
        app.update();

        let mut query = app.world_mut().query::<&SkillUnitCell>();
        let cell = query.iter(app.world()).next().expect("cell");
        assert_eq!(cell.hp, 25);
    }

    #[test]
    fn unknown_group_on_despawn_is_a_noop() {
        let mut app = test_app(seeded_catalog());
        app.world_mut().write_message(SkillUnitSpawned {
            group: group(1, STORM_GUST, vec![cell(100, 40, 50, true)]),
        });
        app.update();

        app.world_mut().write_message(SkillUnitDespawned {
            group_id: 999,
            cell_ids: vec![100],
            reason: Default::default(),
        });
        app.update();

        assert_eq!(roots(&mut app), 1, "unrelated group untouched");
        assert_eq!(cell_count(&mut app), 1);
    }

    #[test]
    fn group_anchor_yields_exactly_one_effect() {
        let mut app = test_app(seeded_catalog());
        app.world_mut().write_message(SkillUnitSpawned {
            group: group(
                1,
                STORM_GUST,
                vec![
                    cell(100, 40, 50, true),
                    cell(101, 41, 50, true),
                    cell(102, 42, 50, true),
                ],
            ),
        });
        app.update();
        assert_eq!(
            effects(&mut app),
            1,
            "group anchor spawns one effect on the root regardless of cell count"
        );
    }

    #[test]
    fn cell_anchor_yields_one_effect_per_visible_cell() {
        const ICE_WALL: u32 = 87;
        let mut app = test_app(cell_anchored_catalog(ICE_WALL));
        app.world_mut().write_message(SkillUnitSpawned {
            group: group(
                1,
                ICE_WALL,
                vec![
                    cell(100, 40, 50, true),
                    cell(101, 41, 50, true),
                    cell(102, 42, 50, false), // not visible: no effect
                ],
            ),
        });
        app.update();
        assert_eq!(
            effects(&mut app),
            2,
            "one effect per visible cell, none for the hidden cell"
        );
    }

    #[test]
    fn cell_anchored_vfx_only_spawns_one_placeholder_per_visible_cell() {
        const ICE_WALL: u32 = 87;
        let mut app = test_app(cell_anchored_vfx_catalog(ICE_WALL));
        app.world_mut().write_message(SkillUnitSpawned {
            group: group(
                1,
                ICE_WALL,
                vec![
                    cell(100, 40, 50, true),
                    cell(101, 41, 50, true),
                    cell(102, 42, 50, false), // not visible: no placeholder
                ],
            ),
        });
        app.update();

        assert_eq!(
            placeholders(&mut app),
            2,
            "one persistent prism per visible cell, none for the hidden cell or STR"
        );
        assert_eq!(
            effects(&mut app),
            0,
            "vfx-only descriptor spawns no STR effect"
        );
    }

    #[test]
    fn targetable_cell_registers_non_targetable_does_not() {
        let mut app = test_app(seeded_catalog());
        app.world_mut().write_message(SkillUnitSpawned {
            group: group(
                1,
                STORM_GUST,
                vec![targetable_cell(100, 40, 50, true), cell(101, 41, 50, true)],
            ),
        });
        app.update();

        let registry = app.world().resource::<EntityRegistry>();
        assert!(
            registry.get_entity(100).is_some(),
            "targetable cell registers its cell_id"
        );
        assert!(
            registry.get_entity(101).is_none(),
            "non-targetable cell does not register"
        );
    }

    #[test]
    fn despawn_unregisters_targetable_cell() {
        let mut app = test_app(seeded_catalog());
        app.world_mut().write_message(SkillUnitSpawned {
            group: group(1, STORM_GUST, vec![targetable_cell(100, 40, 50, true)]),
        });
        app.update();
        assert!(app
            .world()
            .resource::<EntityRegistry>()
            .get_entity(100)
            .is_some());

        app.world_mut().write_message(SkillUnitDespawned {
            group_id: 1,
            cell_ids: vec![100],
            reason: Default::default(),
        });
        app.update();

        assert!(
            app.world()
                .resource::<EntityRegistry>()
                .get_entity(100)
                .is_none(),
            "despawn unregisters the targetable cell"
        );
    }

    #[test]
    fn duplicate_spawn_unregisters_old_targetable_cells() {
        let mut app = test_app(seeded_catalog());
        app.world_mut().write_message(SkillUnitSpawned {
            group: group(1, STORM_GUST, vec![targetable_cell(100, 40, 50, true)]),
        });
        app.update();
        assert!(app
            .world()
            .resource::<EntityRegistry>()
            .get_entity(100)
            .is_some());

        // Group re-entering view (or a duplicate spawn) with a different cell
        // id: the old targetable cell's registration must not survive the replace.
        app.world_mut().write_message(SkillUnitSpawned {
            group: group(1, STORM_GUST, vec![targetable_cell(200, 40, 50, true)]),
        });
        app.update();

        let registry = app.world().resource::<EntityRegistry>();
        assert!(
            registry.get_entity(100).is_none(),
            "old cell's registration is dropped on replace"
        );
        assert!(registry.get_entity(200).is_some(), "new cell registers");
    }
}
