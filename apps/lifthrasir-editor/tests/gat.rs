//! A GAT stroke is one undoable map data edit; the saved GAT carries the
//! painted raw types and undo rebuilds the chunked overlay.

mod common;

use bevy::prelude::*;
use lifthrasir_editor::{
    map::{
        document::collect_document,
        layout::MapLayout,
        tools::gat::{
            AutoGatOptions, GatBrush, GatOverlayChunk, auto_gat, end_stroke, paint_cells,
        },
    },
    shell::undo::{UndoStack, redo, undo},
};
use lifthrasir_runtime::world::LifMapData;
use ro_formats::gat::GatCellType;

/// `GRAT`, version, width, height; then 4 heights and the type per cell.
const HEADER_BYTES: usize = 14;
const CELL_BYTES: usize = 20;

fn raw_types(app: &App, layout: &MapLayout) -> Vec<u32> {
    let data = app.world().get::<LifMapData>(layout.root).unwrap();
    data.altitude
        .cells
        .iter()
        .map(|cell| cell.cell_type.raw())
        .collect()
}

/// The vertices of every overlay chunk.
fn overlay_vertices(app: &mut App) -> usize {
    let world = app.world_mut();
    let handles: Vec<Handle<Mesh>> = world
        .query_filtered::<&Mesh3d, With<GatOverlayChunk>>()
        .iter(world)
        .map(|mesh| mesh.0.clone())
        .collect();
    let meshes = world.resource::<Assets<Mesh>>();
    handles
        .iter()
        .map(|handle| meshes.get(handle).unwrap().count_vertices())
        .sum()
}

#[test]
fn a_stroke_saves_its_raw_types_and_undoes_in_one_step() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let original = raw_types(&app, &layout);
    let (width, height) = {
        let altitude = &app.world().get::<LifMapData>(layout.root).unwrap().altitude;
        (altitude.width, altitude.height)
    };
    let center = (width / 2, height / 2);
    let brush = GatBrush {
        radius: 1,
        raw_type: 5,
    };
    let index = (center.1 * width + center.0) as usize;
    assert_ne!(original[index], brush.raw_type);

    let before = app.world().get::<LifMapData>(layout.root).unwrap().clone();
    let mut data = app.world_mut().get_mut::<LifMapData>(layout.root).unwrap();
    paint_cells(&mut data.altitude, center, &brush);
    end_stroke(app.world_mut(), before);

    assert_eq!(
        app.world().resource::<UndoStack>().undo_label(),
        Some("Edit map data")
    );
    let bytes = collect_document(app.world_mut(), &layout, false)
        .unwrap()
        .gat
        .to_bytes()
        .unwrap();
    let at = HEADER_BYTES + index * CELL_BYTES + 16;
    assert_eq!(&bytes[at..at + 4], &brush.raw_type.to_le_bytes());

    undo(app.world_mut());
    assert_eq!(raw_types(&app, &layout), original);
    assert!(app.world().resource::<UndoStack>().is_pristine());
    assert_eq!(overlay_vertices(&mut app), 4 * (width * height) as usize);
}

#[test]
fn a_stroke_that_changes_nothing_is_not_recorded() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let before = app.world().get::<LifMapData>(layout.root).unwrap().clone();

    end_stroke(app.world_mut(), before);

    assert!(app.world().resource::<UndoStack>().is_pristine());
}

#[test]
fn auto_gat_is_one_undoable_map_data_edit() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    {
        let mut data = app.world_mut().get_mut::<LifMapData>(layout.root).unwrap();
        data.altitude.cells[0].height = [100.0; 4];
        data.altitude.cells[0].cell_type = GatCellType::from(5);
    }
    let corrupted = app
        .world()
        .get::<LifMapData>(layout.root)
        .unwrap()
        .altitude
        .to_bytes()
        .unwrap();

    auto_gat(app.world_mut(), AutoGatOptions::default());
    let regenerated = app
        .world()
        .get::<LifMapData>(layout.root)
        .unwrap()
        .altitude
        .to_bytes()
        .unwrap();
    assert_ne!(regenerated, corrupted);
    assert_eq!(
        app.world().resource::<UndoStack>().undo_label(),
        Some("Edit map data")
    );

    undo(app.world_mut());
    assert_eq!(
        app.world()
            .get::<LifMapData>(layout.root)
            .unwrap()
            .altitude
            .to_bytes()
            .unwrap(),
        corrupted
    );
    redo(app.world_mut());
    assert_eq!(
        app.world()
            .get::<LifMapData>(layout.root)
            .unwrap()
            .altitude
            .to_bytes()
            .unwrap(),
        regenerated
    );
}
