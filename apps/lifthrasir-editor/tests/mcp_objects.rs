//! Object tools over MCP: every edit is one `Agent: <tool>` undo entry that
//! a single undo reverts, and none of them touches the human's selection or
//! tool mode.

mod common;
mod mcp_support;

use bevy::prelude::*;
use lifthrasir_editor::mcp::call::AgentCall;
use lifthrasir_editor::{
    map::{
        document::collect_document,
        layout::MapLayout,
        tools::{ToolMode, gat::GatBrush},
    },
    mcp::call::{
        AddLightParams, AgentError, AssetParam, Call, LightKindParam, LightPatch, ListNodesParams,
        MapSettingsParams, NodeKindParam, NodeRef, PlaceParams, UpdateNodeParams,
    },
    shell::{
        node::EditorNodeId,
        selection::{Selection, delete_selected},
        undo::{UndoStack, undo},
    },
};
use lifthrasir_runtime::world::{LifMapData, LifPropRef};
use lifthrasir_tooling::converters::map::writer::write_map_glb;
use mcp_support::{call, mcp_app};
use tokio::sync::mpsc::UnboundedSender;

/// The open map as the bytes a save would write.
fn saved_bytes(app: &mut App, layout: &MapLayout) -> Vec<u8> {
    let doc = collect_document(app.world_mut(), layout, false).unwrap();
    write_map_glb(&doc).unwrap()
}

/// Runs `call`, checks it left exactly one `Agent: <tool>` entry and that one
/// undo restores the map; returns the call's JSON.
fn one_undoable_step(
    app: &mut App,
    tx: &UnboundedSender<AgentCall>,
    layout: &MapLayout,
    tool: Call,
) -> serde_json::Value {
    let label = format!("Agent: {}", tool.name());
    let before = saved_bytes(app, layout);
    let depth = app.world().resource::<UndoStack>().depth();

    let json = call(app, tx, tool).unwrap().json;

    let stack = app.world().resource::<UndoStack>();
    assert_eq!(stack.depth(), depth + 1, "{label}");
    assert_eq!(stack.undo_label(), Some(label.as_str()));
    assert_ne!(saved_bytes(app, layout), before, "{label} changed nothing");
    undo(app.world_mut());
    assert_eq!(saved_bytes(app, layout), before, "{label} did not undo");
    json
}

fn a_prop(app: &mut App) -> EditorNodeId {
    app.world_mut()
        .query_filtered::<&EditorNodeId, With<LifPropRef>>()
        .iter(app.world())
        .copied()
        .next()
        .unwrap()
}

fn tree() -> AssetParam {
    AssetParam::Model {
        uri: "ro://models/prontera/tree01.glb".into(),
    }
}

#[test]
fn place_asset_is_one_undoable_step() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    let layout = common::open_fixture(&mut app);

    let placed = one_undoable_step(
        &mut app,
        &tx,
        &layout,
        Call::PlaceAsset(PlaceParams {
            asset: tree(),
            cell: Some([1, 2]),
            world: None,
        }),
    );

    assert!(placed["node_id"].is_u64(), "{placed}");
    assert_eq!(placed["cell"], serde_json::json!([1, 2]));
}

#[test]
fn add_light_is_one_undoable_step() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    let layout = common::open_fixture(&mut app);

    one_undoable_step(
        &mut app,
        &tx,
        &layout,
        Call::AddLight(AddLightParams {
            kind: LightKindParam::Spot,
            cell: Some([2, 2]),
            world: None,
            height: None,
        }),
    );
}

#[test]
fn update_node_is_one_undoable_step() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let id = a_prop(&mut app);

    one_undoable_step(
        &mut app,
        &tx,
        &layout,
        Call::UpdateNode(Box::new(UpdateNodeParams {
            node_id: id.0,
            name: Some("moved tree".into()),
            cell: Some([3, 3]),
            scale: Some([2.0, 2.0, 2.0]),
            ..Default::default()
        })),
    );
}

#[test]
fn delete_node_is_one_undoable_step() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let id = a_prop(&mut app);

    one_undoable_step(
        &mut app,
        &tx,
        &layout,
        Call::DeleteNode(NodeRef { node_id: id.0 }),
    );
}

#[test]
fn set_map_settings_is_one_undoable_step() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let exposure = app
        .world()
        .get::<LifMapData>(layout.root)
        .unwrap()
        .meta
        .exposure_ev100;

    one_undoable_step(
        &mut app,
        &tx,
        &layout,
        Call::SetMapSettings(MapSettingsParams {
            exposure_ev100: Some(exposure + 1.0),
            ..Default::default()
        }),
    );
}

#[test]
fn editing_the_sun_is_one_undoable_step() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let suns = call(
        &mut app,
        &tx,
        Call::ListNodes(ListNodesParams {
            kind: Some(NodeKindParam::Sun),
            ..Default::default()
        }),
    )
    .unwrap()
    .json;
    let sun = suns["nodes"][0]["id"].as_u64().unwrap();

    let sun_json = one_undoable_step(
        &mut app,
        &tx,
        &layout,
        Call::UpdateNode(Box::new(UpdateNodeParams {
            node_id: sun,
            light: Some(LightPatch {
                elevation_deg: Some(30.0),
                lux: Some(5000.0),
                ..Default::default()
            }),
            ..Default::default()
        })),
    );
    assert_eq!(sun_json["payload"]["lux"], 5000.0);

    let moved = call(
        &mut app,
        &tx,
        Call::UpdateNode(Box::new(UpdateNodeParams {
            node_id: sun,
            translation: Some([0.0, 0.0, 0.0]),
            ..Default::default()
        })),
    );
    assert!(matches!(moved, Err(AgentError::Invalid(_))), "{moved:?}");
}

#[test]
fn agent_placement_keeps_the_human_selection_and_tool() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    common::open_fixture(&mut app);
    let selected = Some(a_prop(&mut app));
    let mode = ToolMode::GatPaint(GatBrush::default());
    app.world_mut().resource_mut::<Selection>().0 = selected;
    *app.world_mut().resource_mut::<ToolMode>() = mode.clone();

    call(
        &mut app,
        &tx,
        Call::PlaceAsset(PlaceParams {
            asset: tree(),
            cell: Some([0, 0]),
            world: None,
        }),
    )
    .unwrap();
    call(
        &mut app,
        &tx,
        Call::AddLight(AddLightParams {
            kind: LightKindParam::Point,
            cell: Some([1, 1]),
            world: None,
            height: None,
        }),
    )
    .unwrap();

    assert_eq!(app.world().resource::<Selection>().0, selected);
    assert_eq!(*app.world().resource::<ToolMode>(), mode);
}

#[test]
fn update_of_a_deleted_node_is_not_found() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    common::open_fixture(&mut app);
    let id = a_prop(&mut app);
    app.world_mut().resource_mut::<Selection>().0 = Some(id);
    delete_selected(app.world_mut());
    let depth = app.world().resource::<UndoStack>().depth();

    let result = call(
        &mut app,
        &tx,
        Call::UpdateNode(Box::new(UpdateNodeParams {
            node_id: id.0,
            cell: Some([1, 1]),
            ..Default::default()
        })),
    );

    assert_eq!(
        result.unwrap_err(),
        AgentError::NotFound(format!("node {}", id.0))
    );
    assert_eq!(app.world().resource::<UndoStack>().depth(), depth);
}

#[test]
fn placing_a_missing_asset_is_not_found() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let children = |app: &App| app.world().get::<Children>(layout.map_node).unwrap().len();
    let before = children(&app);

    let result = call(
        &mut app,
        &tx,
        Call::PlaceAsset(PlaceParams {
            asset: AssetParam::Model {
                uri: "ro://models/does_not_exist.glb".into(),
            },
            cell: Some([0, 0]),
            world: None,
        }),
    );

    assert!(matches!(result, Err(AgentError::NotFound(_))), "{result:?}");
    assert_eq!(children(&app), before);
}

#[test]
fn edits_without_a_map_are_refused() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());

    let result = call(
        &mut app,
        &tx,
        Call::PlaceAsset(PlaceParams {
            asset: tree(),
            cell: Some([0, 0]),
            world: None,
        }),
    );

    assert_eq!(result.unwrap_err(), AgentError::NoMapOpen);
}

#[test]
fn invalid_spot_angles_are_rejected() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    common::open_fixture(&mut app);
    let added = call(
        &mut app,
        &tx,
        Call::AddLight(AddLightParams {
            kind: LightKindParam::Spot,
            cell: Some([2, 2]),
            world: None,
            height: None,
        }),
    )
    .unwrap()
    .json;
    let depth = app.world().resource::<UndoStack>().depth();

    let result = call(
        &mut app,
        &tx,
        Call::UpdateNode(Box::new(UpdateNodeParams {
            node_id: added["node_id"].as_u64().unwrap(),
            light: Some(LightPatch {
                inner_angle_deg: Some(40.0),
                outer_angle_deg: Some(20.0),
                ..Default::default()
            }),
            ..Default::default()
        })),
    );

    assert!(matches!(result, Err(AgentError::Invalid(_))), "{result:?}");
    assert_eq!(app.world().resource::<UndoStack>().depth(), depth);
}
