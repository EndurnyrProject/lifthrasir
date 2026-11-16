mod common;
mod mcp_support;

use bevy::prelude::*;
use lifthrasir_editor::{
    map::{document::collect_document, layout::MapLayout, terrain::terrain_heights},
    mcp::call::{
        AgentCall, AgentError, Call, ColorParams, PaintTilesParams, SculptOpParam, SculptParams,
        WallParams,
    },
    shell::{
        browser::AssetIndex,
        undo::{UndoStack, undo},
    },
};
use mcp_support::{call, mcp_app};
use tokio::sync::mpsc::UnboundedSender;

const DIRT: &str = "ro://ground/tex/dirt.ktx2";

fn document(app: &mut App, layout: &MapLayout, ignore_textures: bool) -> String {
    let mut doc = collect_document(app.world_mut(), layout, false).unwrap();
    if ignore_textures {
        doc.textures.clear();
    }
    format!("{doc:?}")
}

fn run_edit(app: &mut App, tx: &UnboundedSender<AgentCall>, tool: Call) {
    let label = format!("Agent: {}", tool.name());
    let depth = app.world().resource::<UndoStack>().depth();
    assert_eq!(call(app, tx, tool).unwrap().json["changed"], true);
    let stack = app.world().resource::<UndoStack>();
    assert_eq!(stack.depth(), depth + 1);
    assert_eq!(stack.undo_label(), Some(label.as_str()));
}

#[test]
fn sculpt_at_the_map_edge_is_clipped() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let before = document(&mut app, &layout, false);
    let heights = terrain_heights(app.world(), &layout).get(0, 0);
    run_edit(
        &mut app,
        &tx,
        Call::Sculpt(SculptParams {
            center: [0, 0],
            radius_cells: 4,
            op: SculptOpParam::Raise,
            strength: 10.0,
        }),
    );
    assert_ne!(terrain_heights(app.world(), &layout).get(0, 0), heights);
    undo(app.world_mut());
    assert_eq!(document(&mut app, &layout, false), before);
}

#[test]
fn sculpt_rejects_out_of_range_strength_without_undo_entry() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    common::open_fixture(&mut app);
    let depth = app.world().resource::<UndoStack>().depth();
    let result = call(
        &mut app,
        &tx,
        Call::Sculpt(SculptParams {
            center: [0, 0],
            radius_cells: 4,
            op: SculptOpParam::Raise,
            strength: 3e38,
        }),
    );
    assert!(matches!(result, Err(AgentError::Invalid(_))));
    assert_eq!(app.world().resource::<UndoStack>().depth(), depth);
}

#[test]
fn paint_tiles_is_one_undoable_step() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    common::give_chunks_pbr_materials(&mut app);
    app.world_mut()
        .resource_mut::<AssetIndex>()
        .ground_textures
        .push(DIRT.into());
    let before = document(&mut app, &layout, true);
    run_edit(
        &mut app,
        &tx,
        Call::PaintTiles(PaintTilesParams {
            tiles: vec![[0, 0]],
            texture: DIRT.into(),
            rotation: 0,
            flip: false,
        }),
    );
    let doc = collect_document(app.world_mut(), &layout, false).unwrap();
    assert!(doc.terrain.iter().any(|p| p.texture == "dirt"));
    undo(app.world_mut());
    assert_eq!(document(&mut app, &layout, true), before);
}

#[test]
fn add_wall_is_one_undoable_step() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    common::give_chunks_pbr_materials(&mut app);
    app.world_mut()
        .resource_mut::<AssetIndex>()
        .ground_textures
        .push(DIRT.into());
    let before = document(&mut app, &layout, true);
    let quads = |app: &mut App| {
        collect_document(app.world_mut(), &layout, false)
            .unwrap()
            .terrain
            .iter()
            .map(|p| p.indices.len() / 6)
            .sum::<usize>()
    };
    let count = quads(&mut app);
    run_edit(
        &mut app,
        &tx,
        Call::AddWall(WallParams {
            a: [0, 0],
            b: [0, 1],
            texture: DIRT.into(),
        }),
    );
    assert_eq!(quads(&mut app), count + 1);
    undo(app.world_mut());
    assert_eq!(document(&mut app, &layout, true), before);
}

#[test]
fn paint_vertex_color_is_one_undoable_step() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let before = document(&mut app, &layout, false);
    let colors = collect_document(app.world_mut(), &layout, false)
        .unwrap()
        .terrain[0]
        .colors
        .clone();
    run_edit(
        &mut app,
        &tx,
        Call::PaintVertexColor(ColorParams {
            center: [0, 0],
            radius_cells: 4,
            color: [0.0, 0.0, 0.0, 1.0],
            strength: 1.0,
        }),
    );
    assert_ne!(
        collect_document(app.world_mut(), &layout, false)
            .unwrap()
            .terrain[0]
            .colors,
        colors
    );
    undo(app.world_mut());
    assert_eq!(document(&mut app, &layout, false), before);
}

#[test]
fn paint_tiles_with_a_non_ground_texture_is_not_found() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    common::open_fixture(&mut app);
    let result = call(
        &mut app,
        &tx,
        Call::PaintTiles(PaintTilesParams {
            tiles: vec![[0, 0]],
            texture: "ro://models/prontera/tree01.glb".into(),
            rotation: 0,
            flip: false,
        }),
    );
    assert!(matches!(result, Err(AgentError::NotFound(_))));
}

#[test]
fn add_wall_between_non_adjacent_tiles_is_invalid() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let before = document(&mut app, &layout, false);
    let depth = app.world().resource::<UndoStack>().depth();
    let result = call(
        &mut app,
        &tx,
        Call::AddWall(WallParams {
            a: [0, 0],
            b: [1, 1],
            texture: "ro://ground/tex/grass01.ktx2".into(),
        }),
    );
    assert!(matches!(result, Err(AgentError::Invalid(_))));
    assert_eq!(app.world().resource::<UndoStack>().depth(), depth);
    assert_eq!(document(&mut app, &layout, false), before);
}

#[test]
fn paint_water_on_a_dry_map_is_one_undoable_step() {
    use lifthrasir_editor::mcp::call::WaterParams;
    use lifthrasir_runtime::world::LifMapData;
    let folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(folder.path());
    let layout = common::open_fixture(&mut app);
    {
        let mut data = app.world_mut().get_mut::<LifMapData>(layout.root).unwrap();
        data.water = None;
        data.water_tiles.clear();
    }
    let before = document(&mut app, &layout, false);
    run_edit(
        &mut app,
        &tx,
        Call::PaintWater(WaterParams {
            tiles: vec![[0, 0]],
            add: true,
        }),
    );
    assert_eq!(
        app.world()
            .get::<LifMapData>(layout.root)
            .unwrap()
            .water_tiles,
        vec![(0, 0)]
    );
    undo(app.world_mut());
    assert_eq!(document(&mut app, &layout, false), before);
}

#[test]
fn paint_gat_is_one_undoable_step() {
    use lifthrasir_editor::mcp::{call::GatParams, coords::Region};
    use lifthrasir_runtime::world::LifMapData;
    let folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(folder.path());
    let layout = common::open_fixture(&mut app);
    let before = document(&mut app, &layout, false);
    run_edit(
        &mut app,
        &tx,
        Call::PaintGat(GatParams {
            region: Region::Cells(vec![[0, 0]]),
            raw_type: 5,
        }),
    );
    assert_eq!(
        app.world()
            .get::<LifMapData>(layout.root)
            .unwrap()
            .altitude
            .cells[0]
            .cell_type
            .raw(),
        5
    );
    undo(app.world_mut());
    assert_eq!(document(&mut app, &layout, false), before);
}

#[test]
fn auto_gat_is_one_undoable_step() {
    use lifthrasir_editor::mcp::call::AutoGatParams;
    use lifthrasir_runtime::world::LifMapData;
    use ro_formats::gat::GatCellType;
    let folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(folder.path());
    let layout = common::open_fixture(&mut app);
    app.world_mut()
        .get_mut::<LifMapData>(layout.root)
        .unwrap()
        .altitude
        .cells[0]
        .cell_type = GatCellType::from(5);
    let before = document(&mut app, &layout, false);
    run_edit(
        &mut app,
        &tx,
        Call::AutoGat(AutoGatParams {
            set_heights: true,
            set_walkability: true,
            block_under_water: false,
            block_props: false,
        }),
    );
    assert_ne!(
        app.world()
            .get::<LifMapData>(layout.root)
            .unwrap()
            .altitude
            .cells[0]
            .cell_type
            .raw(),
        5
    );
    undo(app.world_mut());
    assert_eq!(document(&mut app, &layout, false), before);
}

#[test]
fn paint_gat_with_an_unknown_raw_type_is_invalid() {
    use lifthrasir_editor::mcp::{call::GatParams, coords::Region};
    let folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(folder.path());
    let layout = common::open_fixture(&mut app);
    let before = document(&mut app, &layout, false);
    let depth = app.world().resource::<UndoStack>().depth();
    assert!(matches!(
        call(
            &mut app,
            &tx,
            Call::PaintGat(GatParams {
                region: Region::Cells(vec![[0, 0]]),
                raw_type: 7
            })
        ),
        Err(AgentError::Invalid(_))
    ));
    assert_eq!(app.world().resource::<UndoStack>().depth(), depth);
    assert_eq!(document(&mut app, &layout, false), before);
}

#[test]
fn removing_water_from_a_dry_map_is_invalid() {
    use lifthrasir_editor::mcp::call::WaterParams;
    use lifthrasir_runtime::world::LifMapData;
    let folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(folder.path());
    let layout = common::open_fixture(&mut app);
    {
        let mut data = app.world_mut().get_mut::<LifMapData>(layout.root).unwrap();
        data.water = None;
        data.water_tiles.clear();
    }
    let before = document(&mut app, &layout, false);
    let depth = app.world().resource::<UndoStack>().depth();
    assert!(matches!(
        call(
            &mut app,
            &tx,
            Call::PaintWater(WaterParams {
                tiles: vec![[0, 0]],
                add: false
            })
        ),
        Err(AgentError::Invalid(_))
    ));
    assert_eq!(app.world().resource::<UndoStack>().depth(), depth);
    assert_eq!(document(&mut app, &layout, false), before);
}

#[test]
fn water_tiles_outside_the_ground_grid_are_rejected_without_changes() {
    use lifthrasir_editor::mcp::call::WaterParams;
    let folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(folder.path());
    let layout = common::open_fixture(&mut app);
    let before = document(&mut app, &layout, false);
    let depth = app.world().resource::<UndoStack>().depth();
    assert!(matches!(
        call(
            &mut app,
            &tx,
            Call::PaintWater(WaterParams {
                tiles: vec![[0, 0], [2, 0]],
                add: true
            })
        ),
        Err(AgentError::OutOfBounds(_))
    ));
    assert_eq!(app.world().resource::<UndoStack>().depth(), depth);
    assert_eq!(document(&mut app, &layout, false), before);
}
