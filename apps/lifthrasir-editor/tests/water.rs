//! Water edits are undoable map data edits that rebuild the water surfaces;
//! the terrain heights read back from the meshes are the converter's.

mod common;

use bevy::prelude::*;
use lifthrasir_editor::{
    map::{
        layout::MapLayout,
        terrain::terrain_heights,
        tools::water::{add_water, commit_water, new_water, recompute_tiles},
    },
    shell::undo::undo,
};
use lifthrasir_runtime::world::{
    LifMapData, SceneOwner,
    water::{WaterLoadingState, WaterSurface},
};
use std::time::{Duration, Instant};

fn data(app: &App, layout: &MapLayout) -> LifMapData {
    app.world().get::<LifMapData>(layout.root).unwrap().clone()
}

/// The levels of the root's water surfaces once no water load is pending.
fn settled_levels(app: &mut App, root: Entity) -> Vec<f32> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.world().get::<WaterLoadingState>(root).is_some() {
        assert!(
            Instant::now() < deadline,
            "the water never finished loading"
        );
        app.update();
        std::thread::sleep(Duration::from_millis(2));
    }
    app.update();
    let world = app.world_mut();
    world
        .query::<(&SceneOwner, &WaterSurface)>()
        .iter(world)
        .filter(|(owner, _)| owner.0 == root)
        .map(|(_, surface)| surface.water_level)
        .collect()
}

#[test]
fn terrain_heights_are_the_fixture_gnd_heights() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);

    let heights = terrain_heights(app.world(), &layout);

    assert_eq!((heights.width, heights.height), (2, 2));
    assert_eq!(
        heights.cells,
        vec![[-10.0; 4], [12.0; 4], [12.0; 4], [-10.0; 4]]
    );
}

#[test]
fn a_level_change_and_a_removal_rebuild_the_water_until_undone() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let original = data(&app, &layout);
    assert_eq!(settled_levels(&mut app, layout.root), vec![12.5]);

    let mut raised = original.water.clone().unwrap();
    raised.zones[0].level = 15.0;
    commit_water(app.world_mut(), Some(raised), original.water_tiles.clone());
    assert_eq!(settled_levels(&mut app, layout.root), vec![15.0]);

    commit_water(app.world_mut(), None, Vec::new());
    assert!(settled_levels(&mut app, layout.root).is_empty());
    assert!(data(&app, &layout).water.is_none());

    undo(app.world_mut());
    assert_eq!(settled_levels(&mut app, layout.root), vec![15.0]);
    undo(app.world_mut());
    assert_eq!(settled_levels(&mut app, layout.root), vec![12.5]);
    let restored = data(&app, &layout);
    assert_eq!(restored.water, original.water);
    assert_eq!(restored.water_tiles, original.water_tiles);
}

#[test]
fn recomputed_tiles_match_the_fixture_mask() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let tiles = data(&app, &layout).water_tiles;
    assert_eq!(tiles, vec![(1, 0), (0, 1)]);

    assert_eq!(recompute_tiles(app.world(), &layout), Some(tiles));
}

#[test]
fn added_water_covers_the_gnd_with_no_tiles_and_draws_nothing() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    commit_water(app.world_mut(), None, Vec::new());

    add_water(app.world_mut());

    let added = data(&app, &layout);
    assert_eq!(added.water, Some(new_water(2, 2)));
    assert!(added.water_tiles.is_empty());
    assert!(settled_levels(&mut app, layout.root).is_empty());
}
