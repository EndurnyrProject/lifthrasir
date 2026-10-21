//! Opening a map through the editor prepares it for editing; saving an
//! untouched map writes the same bytes into the loose data folder.

mod common;

use bevy::prelude::*;
use lifthrasir_editor::{
    map::{
        layout::MapLayout,
        open::OpenMap,
        save::{SaveRequest, map_glb_path},
    },
    shell::node::EditorNodeId,
};
use lifthrasir_runtime::world::LifMapData;

#[test]
fn opening_assigns_node_ids_and_applies_map_lighting() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());

    let layout = common::open_fixture(&mut app);

    let world = app.world();
    let children = world.get::<Children>(layout.map_node).unwrap();
    let objects: Vec<Entity> = children
        .iter()
        .filter(|&child| Some(child) != layout.terrain_node)
        .collect();
    assert_eq!(objects.len(), 6);
    assert!(
        objects
            .iter()
            .all(|&node| world.get::<EditorNodeId>(node).is_some())
    );
    let meta = &world.get::<LifMapData>(layout.root).unwrap().meta;
    assert_eq!(
        world.resource::<GlobalAmbientLight>().brightness,
        meta.ambient_brightness
    );
}

#[test]
fn saving_an_untouched_map_writes_the_fixture_bytes_loose() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout: MapLayout = common::open_fixture(&mut app);

    app.world_mut().write_message(SaveRequest { save_as: None });
    app.update();

    let saved = std::fs::read(map_glb_path(data_folder.path(), &layout.name)).unwrap();
    let fixture = std::fs::read(common::fixtures_dir().join("mini_map.glb")).unwrap();
    assert!(saved == fixture, "saved glb differs from the fixture");
}

#[test]
fn save_as_writes_the_new_name_and_renames_the_open_map() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    common::open_fixture(&mut app);

    app.world_mut().write_message(SaveRequest {
        save_as: Some("mini_edit".into()),
    });
    app.update();

    assert!(map_glb_path(data_folder.path(), "mini_edit").exists());
    let world = app.world();
    assert_eq!(world.resource::<MapLayout>().name, "mini_edit");
    let open = world.resource::<OpenMap>();
    assert_eq!(open.name, "mini_edit");
    assert_eq!(open.path, "ro://maps/mini_edit/mini_edit.glb");
}
