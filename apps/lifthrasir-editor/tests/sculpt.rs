//! A sculpt stroke is one undoable step that moves the terrain meshes and
//! rewrites the GAT heights under the touched cells with the retail rule.

mod common;

use bevy::prelude::*;
use lifthrasir_editor::{
    map::{
        layout::MapLayout,
        terrain::{MeshSnapshot, TerrainChunks, terrain_heights},
        tools::sculpt::{SculptBrush, SculptOp, begin_stroke, end_stroke, sculpt_step},
    },
    shell::undo::{UndoStack, redo, undo},
};
use lifthrasir_runtime::world::LifMapData;
use lifthrasir_tooling::converters::map::blank::gat_cell_heights;

fn meshes(app: &App) -> Vec<MeshSnapshot> {
    let chunks = app
        .world()
        .resource::<TerrainChunks>()
        .chunks_in([0, 0], [u32::MAX, u32::MAX]);
    let assets = app.world().resource::<Assets<Mesh>>();
    chunks
        .iter()
        .map(|(_, mesh)| MeshSnapshot::capture(assets.get(mesh).unwrap()))
        .collect()
}

fn gat(app: &App, layout: &MapLayout) -> ro_formats::RoAltitude {
    app.world()
        .get::<LifMapData>(layout.root)
        .unwrap()
        .altitude
        .clone()
}

#[test]
fn a_stroke_rewrites_the_gat_under_its_cells_and_undoes_in_one_step() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let (meshes_before, gat_before) = (meshes(&app), gat(&app, &layout));
    // Only the native (0, 0) corner is in reach: cell (0, 0) alone moves.
    let hit = Vec3::new(1.0, 10.0, -1.0);
    let brush = SculptBrush {
        op: SculptOp::Raise,
        radius: 3.0,
        strength: 20.0,
    };

    begin_stroke(app.world_mut(), hit);
    for _ in 0..3 {
        sculpt_step(app.world_mut(), hit, brush, 0.1);
    }
    end_stroke(app.world_mut());

    assert_eq!(
        app.world().resource::<UndoStack>().undo_label(),
        Some("Edit terrain")
    );
    assert_ne!(meshes(&app), meshes_before);
    let corners = terrain_heights(app.world(), &layout).get(0, 0);
    assert_ne!(corners, [-10.0; 4], "the sculpted corner moved");
    let gat_after = gat(&app, &layout);
    let width = gat_after.width as usize;
    for (index, (after, before)) in gat_after.cells.iter().zip(&gat_before.cells).enumerate() {
        let (x, y) = (index % width, index / width);
        assert_eq!(after.cell_type, before.cell_type, "GAT ({x}, {y}) type");
        if x < 2 && y < 2 {
            assert_eq!(
                after.height,
                gat_cell_heights(corners, x, y),
                "GAT ({x}, {y})"
            );
        } else {
            assert_eq!(
                after.height, before.height,
                "GAT ({x}, {y}) is outside the stroke"
            );
        }
    }

    let meshes_after = meshes(&app);

    undo(app.world_mut());
    assert!(app.world().resource::<UndoStack>().is_pristine());
    assert_eq!(meshes(&app), meshes_before);
    assert_eq!(
        gat(&app, &layout).to_bytes().unwrap(),
        gat_before.to_bytes().unwrap()
    );
    redo(app.world_mut());
    assert_eq!(meshes(&app), meshes_after);
    assert_eq!(
        gat(&app, &layout).to_bytes().unwrap(),
        gat_after.to_bytes().unwrap()
    );
}

#[test]
fn a_stroke_off_the_terrain_records_nothing() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    common::open_fixture(&mut app);
    let far = Vec3::new(500.0, 0.0, -500.0);

    begin_stroke(app.world_mut(), far);
    sculpt_step(app.world_mut(), far, SculptBrush::default(), 0.1);
    end_stroke(app.world_mut());

    assert!(app.world().resource::<UndoStack>().is_pristine());
}
