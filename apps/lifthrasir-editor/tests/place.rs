//! Placing a browser asset adds one undoable object node under the map node,
//! in the shape the runtime builds visuals for and the document collects.

mod common;

use bevy::prelude::*;
use lifthrasir_editor::{
    map::{
        document::collect_document,
        tools::{ToolMode, place::place},
    },
    shell::{browser::AssetRef, node::EditorNodeId, selection::Selection, undo::undo},
};
use lifthrasir_runtime::world::{LifPropRef, PropSceneContext, SceneOwner, props::MapModel};

fn selected_node(app: &mut App) -> Entity {
    let id = app.world().resource::<Selection>().0.unwrap();
    app.world_mut()
        .query::<(Entity, &EditorNodeId)>()
        .iter(app.world())
        .find_map(|(entity, node)| (*node == id).then_some(entity))
        .unwrap()
}

#[test]
fn a_placed_prop_is_a_runtime_prop_node_until_undone() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let children = |app: &App| app.world().get::<Children>(layout.map_node).unwrap().len();
    let before = children(&app);

    place(
        app.world_mut(),
        &AssetRef::Model("ro://models/prontera/tree01.glb".into()),
        Vec3::new(10.0, 0.0, -10.0),
    );

    assert_eq!(children(&app), before + 1);
    let node = selected_node(&mut app);
    let world = app.world();
    assert_eq!(
        world.get::<ChildOf>(node).unwrap().parent(),
        layout.map_node
    );
    assert_eq!(
        world.get::<LifPropRef>(node).unwrap().0.model,
        "ro://models/prontera/tree01.glb"
    );
    assert!(world.get::<PropSceneContext>(node).is_some());
    assert_eq!(world.get::<SceneOwner>(node).unwrap().0, layout.root);
    assert_eq!(
        world.get::<Transform>(node).unwrap().translation,
        Vec3::new(10.0, 0.0, -10.0)
    );

    app.update();
    assert!(app.world().get::<MapModel>(node).is_some());

    undo(app.world_mut());
    assert!(app.world().get_entity(node).is_err());
    assert_eq!(children(&app), before);
}

#[test]
fn a_placed_sound_collects_in_the_native_frame() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let before = collect_document(app.world_mut(), &layout, true)
        .unwrap()
        .sounds
        .len();

    place(
        app.world_mut(),
        &AssetRef::Sound("ro://data/wav/effect/water.wav".into()),
        Vec3::new(10.0, 0.0, -10.0),
    );

    let doc = collect_document(app.world_mut(), &layout, false).unwrap();
    assert_eq!(doc.sounds.len(), before + 1);
    let placed = doc.sounds.last().unwrap();
    assert_eq!(placed.translation, Vec3::new(10.0, 0.0, 10.0));
    assert_eq!(placed.payload.file, "ro://data/wav/effect/water.wav");
    assert_eq!(placed.name, "water");
    assert_eq!(doc.validate(), Ok(()));
}

#[test]
fn placing_returns_to_select_with_the_node_selected() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    common::open_fixture(&mut app);
    let asset = AssetRef::Sound("ro://data/wav/effect/water.wav".into());
    *app.world_mut().resource_mut::<ToolMode>() = ToolMode::Place(asset.clone());

    place(app.world_mut(), &asset, Vec3::ZERO);

    assert_eq!(*app.world().resource::<ToolMode>(), ToolMode::Select);
    assert!(app.world().resource::<Selection>().0.is_some());
}
