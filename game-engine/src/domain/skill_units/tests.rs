//! Message-driven integration tests for the skill-unit systems: the three
//! systems run together in a minimal app, exercised purely via the four
//! lifecycle messages, mirroring how the server drives them.

use bevy::prelude::*;
use lifthrasir_data::{EffectDescriptor, EffectPlacement, GroundAnchor};
use net_contract::dto::{SkillUnitCellFlags, SkillUnitCellState, SkillUnitGroupState};
use net_contract::events::{
    SkillUnitDespawned, SkillUnitSnapshotReceived, SkillUnitSpawned, SkillUnitUpdated,
};
use std::collections::BTreeMap;

use super::components::{SkillUnitCell, SkillUnitGroup};
use super::lifecycle::{despawn_skill_units, update_skill_units};
use super::spawn::spawn_skill_units;
use crate::domain::effects::EffectSprite;
use crate::domain::effects::components::ActiveEffect;
use crate::domain::entities::registry::EntityRegistry;
use crate::infrastructure::effect::{EffectCatalog, EffectDataAsset, LoadedEffectAsset};
use crate::utils::coordinates::spawn_coords_to_world_position;

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

const SAFETY_WALL: u32 = 12; // seeded Ground/Cell anchor with the native STR.
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
            sprite: None,
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
/// spawns a persistent crystal cluster per visible cell instead of an STR.
fn cell_anchored_vfx_catalog(skill_id: u32) -> EffectCatalog {
    let mut skills = BTreeMap::new();
    skills.insert(
        skill_id,
        EffectDescriptor {
            str: None,
            sprite: None,
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

/// Cell-anchored descriptor with a `sprite` stem and NO STR (the Fire Wall /
/// Fire Pillar shape): each visible cell gets a looping SPR/ACT animation.
fn cell_anchored_sprite_catalog(skill_id: u32) -> EffectCatalog {
    let mut skills = BTreeMap::new();
    skills.insert(
        skill_id,
        EffectDescriptor {
            str: None,
            sprite: Some("이팩트/firewall".into()),
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

fn test_app(catalog: EffectCatalog) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::asset::AssetPlugin::default())
        .init_asset::<LoadedEffectAsset>()
        .init_asset::<Mesh>()
        .init_asset::<Image>()
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

/// Crystal clusters carry a `StandardMaterial`; the click colliders on
/// targetable cells are material-less, so this counts only the vfx crystals.
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
fn safety_wall_attaches_native_str_to_its_visible_cell() {
    let mut app = test_app(seeded_catalog());
    app.world_mut().write_message(SkillUnitSpawned {
        group: group(1, SAFETY_WALL, vec![cell(100, 40, 50, true)]),
    });
    app.update();

    let (effect, parent) = app
        .world_mut()
        .query::<(&ActiveEffect, &ChildOf)>()
        .single(app.world())
        .expect("one Safety Wall effect");
    assert!(effect.repeating);
    assert!(
        app.world().get::<SkillUnitCell>(parent.parent()).is_some(),
        "Safety Wall STR must be anchored to its visible unit cell"
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
        "one persistent crystal cluster per visible cell, none for the hidden cell or STR"
    );
    assert_eq!(
        effects(&mut app),
        0,
        "vfx-only descriptor spawns no STR effect"
    );
}

#[test]
fn non_repeating_skill_unit_spawns_no_persistent_visual() {
    const METEOR_STORM: u32 = 83;
    let mut app = test_app(seeded_catalog());
    app.world_mut().write_message(SkillUnitSpawned {
        group: group(1, METEOR_STORM, vec![cell(100, 40, 50, true)]),
    });
    app.update();

    assert_eq!(effects(&mut app), 0, "no STR at the administrative center");
    assert_eq!(
        placeholders(&mut app),
        0,
        "non-repeating VFX must not fall back to an Ice Wall crystal"
    );
}

#[test]
fn cell_anchored_sprite_requests_one_animation_per_visible_cell() {
    const FIRE_WALL: u32 = 18;
    let mut app = test_app(cell_anchored_sprite_catalog(FIRE_WALL));
    app.world_mut().write_message(SkillUnitSpawned {
        group: group(
            1,
            FIRE_WALL,
            vec![
                cell(100, 40, 50, true),
                cell(101, 41, 50, true),
                cell(102, 42, 50, false), // not visible: no flame
            ],
        ),
    });
    app.update();

    let requests: Vec<EffectSprite> = app
        .world_mut()
        .query::<&EffectSprite>()
        .iter(app.world())
        .cloned()
        .collect();
    assert_eq!(
        requests.len(),
        2,
        "one sprite request per visible cell, none for the hidden cell"
    );
    assert!(requests.iter().all(|r| r.path == "이팩트/firewall"));
    assert_eq!(
        effects(&mut app),
        0,
        "sprite descriptor spawns no STR effect"
    );
}

#[test]
fn seeded_firewall_and_firepillar_are_cell_anchored_sprites() {
    let catalog = seeded_catalog();
    for skill_id in [18, 80] {
        let descriptor = catalog.get(skill_id).expect("seeded descriptor");
        assert_eq!(descriptor.ground_anchor, GroundAnchor::Cell);
        assert!(
            descriptor.sprite.is_some(),
            "skill {skill_id} needs a sprite"
        );
        assert!(
            descriptor.str.is_none(),
            "skill {skill_id} must not also STR"
        );
    }
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
    assert!(
        app.world()
            .resource::<EntityRegistry>()
            .get_entity(100)
            .is_some()
    );

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
    assert!(
        app.world()
            .resource::<EntityRegistry>()
            .get_entity(100)
            .is_some()
    );

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
