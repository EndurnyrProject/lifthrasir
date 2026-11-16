//! The tool entry points non-GUI callers (the MCP server) use: they edit and
//! record one undo step like the GUI tools, but never touch the human's
//! selection or tool mode.

mod common;

use bevy::prelude::*;
use lifthrasir_editor::{
    map::tools::{
        ToolMode,
        gat::{GatBrush, paint_region},
        lights::{LightKind, spawn_light},
        place::spawn_placed,
    },
    shell::{
        browser::AssetRef,
        node::{EditorNodeId, find_node},
        selection::Selection,
        undo::{UndoStack, undo},
    },
};
use lifthrasir_runtime::world::LifMapData;

fn children(app: &App, node: Entity) -> usize {
    app.world().get::<Children>(node).map_or(0, |c| c.len())
}

fn raw_type(app: &App, root: Entity, (x, y): (u32, u32)) -> u32 {
    let altitude = &app.world().get::<LifMapData>(root).unwrap().altitude;
    altitude.cells[(y * altitude.width + x) as usize]
        .cell_type
        .raw()
}

#[test]
fn spawn_placed_leaves_selection_and_tool_mode_alone() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let asset = AssetRef::Model("ro://models/prontera/tree01.glb".into());
    let selected = Some(EditorNodeId(12345));
    *app.world_mut().resource_mut::<ToolMode>() = ToolMode::Place(asset.clone());
    app.world_mut().resource_mut::<Selection>().0 = selected;
    let before = children(&app, layout.map_node);

    let id = spawn_placed(app.world_mut(), &asset, Vec3::new(10.0, 0.0, -10.0)).unwrap();

    assert_eq!(children(&app, layout.map_node), before + 1);
    assert!(find_node(app.world_mut(), id).is_some());
    assert_eq!(*app.world().resource::<ToolMode>(), ToolMode::Place(asset));
    assert_eq!(app.world().resource::<Selection>().0, selected);
    undo(app.world_mut());
    assert_eq!(children(&app, layout.map_node), before);
}

#[test]
fn spawn_light_places_the_light_at_the_given_point() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    common::open_fixture(&mut app);
    let mode = ToolMode::GatPaint(GatBrush::default());
    *app.world_mut().resource_mut::<ToolMode>() = mode.clone();
    let at = Vec3::new(10.0, 25.0, -10.0);

    let id = spawn_light(app.world_mut(), LightKind::Point, at).unwrap();

    let node = find_node(app.world_mut(), id).unwrap();
    assert_eq!(app.world().get::<Transform>(node).unwrap().translation, at);
    assert_eq!(*app.world().resource::<ToolMode>(), mode);
    assert_eq!(app.world().resource::<Selection>().0, None);
}

#[test]
fn paint_region_records_one_step_and_undo_restores_types() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let cells = [(0, 0), (1, 0), (2, 1)];
    let original: Vec<u32> = cells
        .iter()
        .map(|&cell| raw_type(&app, layout.root, cell))
        .collect();
    let painted = 5;
    assert!(original.iter().all(|&raw| raw != painted));

    paint_region(app.world_mut(), &cells, painted);

    assert_eq!(app.world().resource::<UndoStack>().depth(), 1);
    for &cell in &cells {
        assert_eq!(raw_type(&app, layout.root, cell), painted);
    }
    undo(app.world_mut());
    let restored: Vec<u32> = cells
        .iter()
        .map(|&cell| raw_type(&app, layout.root, cell))
        .collect();
    assert_eq!(restored, original);
}

#[test]
fn paint_region_with_unchanged_types_records_nothing() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let current = raw_type(&app, layout.root, (0, 0));

    paint_region(app.world_mut(), &[(0, 0)], current);

    assert!(app.world().resource::<UndoStack>().is_pristine());
}
