//! Query tools over MCP: the map summary, per-cell region data, nodes and the
//! asset catalog, read in the same grid the edit tools take.

mod common;
mod mcp_support;

use lifthrasir_editor::{
    map::document::collect_document,
    mcp::call::{
        AgentError, AssetKindParam, Call, ListAssetsParams, ListNodesParams, NodeKindParam,
        NodeRef, RegionQuery,
    },
};
use lifthrasir_runtime::world::LifMapData;
use mcp_support::{call, mcp_app};

#[test]
fn summary_matches_the_fixture() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let doc = collect_document(app.world_mut(), &layout, true).unwrap();
    let altitude = &app.world().get::<LifMapData>(layout.root).unwrap().altitude;
    let (width, height) = (altitude.width, altitude.height);

    let summary = call(&mut app, &tx, Call::MapSummary).unwrap().json;

    assert_eq!(summary["cells"], serde_json::json!([width, height]));
    assert_eq!(summary["tiles"], serde_json::json!([width / 2, height / 2]));
    assert_eq!(summary["nodes"]["prop"], doc.props.len());
    assert_eq!(summary["nodes"]["sound"], doc.sounds.len());
    assert_eq!(summary["nodes"]["effect"], doc.effects.len());
}

#[test]
fn query_region_returns_every_cell_once() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let altitude = app
        .world()
        .get::<LifMapData>(layout.root)
        .unwrap()
        .altitude
        .clone();

    let region = call(
        &mut app,
        &tx,
        Call::QueryRegion(RegionQuery {
            min: [0, 0],
            max: [3, 2],
        }),
    )
    .unwrap()
    .json;

    let cells = region["cells"].as_array().unwrap();
    assert_eq!(cells.len(), 12);
    for cell in cells {
        let [x, y] = [
            cell["cell"][0].as_u64().unwrap(),
            cell["cell"][1].as_u64().unwrap(),
        ];
        let expected = altitude.cells[(y * u64::from(altitude.width) + x) as usize]
            .cell_type
            .raw();
        assert_eq!(cell["gat"]["raw"], expected, "{cell}");
        assert!(cell["tile_texture"].is_string(), "{cell}");
    }
}

#[test]
fn query_region_over_64_is_invalid() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    common::open_fixture(&mut app);

    let result = call(
        &mut app,
        &tx,
        Call::QueryRegion(RegionQuery {
            min: [0, 0],
            max: [64, 0],
        }),
    );

    assert!(matches!(result, Err(AgentError::Invalid(_))), "{result:?}");
}

#[test]
fn query_region_with_a_huge_max_is_rejected_without_panicking() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    common::open_fixture(&mut app);

    let result = call(
        &mut app,
        &tx,
        Call::QueryRegion(RegionQuery {
            min: [0, 0],
            max: [u32::MAX, 0],
        }),
    );

    assert!(matches!(result, Err(AgentError::Invalid(_))), "{result:?}");
}

#[test]
fn list_nodes_filters_by_kind_and_every_id_resolves() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    common::open_fixture(&mut app);

    let all = call(&mut app, &tx, Call::ListNodes(ListNodesParams::default()))
        .unwrap()
        .json;
    let props = call(
        &mut app,
        &tx,
        Call::ListNodes(ListNodesParams {
            kind: Some(NodeKindParam::Prop),
            ..Default::default()
        }),
    )
    .unwrap()
    .json;

    let all = all["nodes"].as_array().unwrap();
    let props = props["nodes"].as_array().unwrap();
    assert!(!props.is_empty() && props.len() < all.len());
    assert!(props.iter().all(|node| node["kind"] == "prop"));
    assert!(all.iter().any(|node| node["kind"] == "sun"));
    for node in all {
        let id = node["id"].as_u64().unwrap();
        let got = call(&mut app, &tx, Call::GetNode(NodeRef { node_id: id }))
            .unwrap()
            .json;
        assert_eq!(got["id"], id);
        assert_eq!(got["kind"], node["kind"]);
    }
}

#[test]
fn list_nodes_filters_by_rect() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    common::open_fixture(&mut app);
    let all = call(&mut app, &tx, Call::ListNodes(ListNodesParams::default()))
        .unwrap()
        .json;
    let first = all["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["cell"].is_array())
        .unwrap()
        .clone();
    let cell = [
        first["cell"][0].as_u64().unwrap() as u32,
        first["cell"][1].as_u64().unwrap() as u32,
    ];

    let inside = call(
        &mut app,
        &tx,
        Call::ListNodes(ListNodesParams {
            min: Some(cell),
            max: Some(cell),
            ..Default::default()
        }),
    )
    .unwrap()
    .json;

    let inside = inside["nodes"].as_array().unwrap();
    assert!(inside.iter().any(|node| node["id"] == first["id"]));
    assert!(inside.iter().all(|node| node["cell"] == first["cell"]));
}

#[test]
fn get_node_unknown_id_is_not_found() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    common::open_fixture(&mut app);

    let result = call(&mut app, &tx, Call::GetNode(NodeRef { node_id: 999_999 }));

    assert!(matches!(result, Err(AgentError::NotFound(_))), "{result:?}");
}

#[test]
fn list_assets_respects_filter_and_limit() {
    let data_folder = tempfile::tempdir().unwrap();
    let (mut app, tx) = mcp_app(data_folder.path());
    app.update();

    let one = call(
        &mut app,
        &tx,
        Call::ListAssets(ListAssetsParams {
            kind: AssetKindParam::GroundTexture,
            filter: Some("grass".into()),
            limit: Some(1),
        }),
    )
    .unwrap()
    .json;
    let none = call(
        &mut app,
        &tx,
        Call::ListAssets(ListAssetsParams {
            kind: AssetKindParam::GroundTexture,
            filter: Some("no_such_texture".into()),
            limit: None,
        }),
    )
    .unwrap()
    .json;

    assert_eq!(
        one["assets"],
        serde_json::json!(["ro://ground/tex/grass01.ktx2"])
    );
    assert_eq!(one["total"], 1);
    assert_eq!(none["total"], 0);
}
