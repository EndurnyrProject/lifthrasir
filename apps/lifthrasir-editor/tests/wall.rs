//! Adding a wall appends one converter-shaped wall quad, refuses a second
//! one on the same edge, saves valid and undoes.

mod common;

use bevy::prelude::*;
use lifthrasir_editor::{
    map::{
        document::collect_document,
        layout::MapTextureList,
        terrain::{MeshSnapshot, TerrainChunks, is_wall_quad, quad_count},
        tools::wall::{WallBrush, add_wall},
    },
    shell::undo::undo,
};

#[test]
fn a_wall_between_two_cells_is_added_once_saved_and_undone() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let texture = app.world().get::<MapTextureList>(layout.root).unwrap().0[0].clone();
    let (_, mesh) = app
        .world()
        .resource::<TerrainChunks>()
        .get([0, 0], &texture.name)
        .unwrap();
    let quads = |app: &App| quad_count(app.world().resource::<Assets<Mesh>>().get(&mesh).unwrap());
    let before = quads(&app);
    let brush = WallBrush {
        texture_uri: texture.uri.clone(),
    };

    add_wall(app.world_mut(), (0, 0), (0, 1), &brush).unwrap();

    assert_eq!(quads(&app), before + 1);
    let assets = app.world().resource::<Assets<Mesh>>();
    let chunk = assets.get(&mesh).unwrap();
    assert!(is_wall_quad(chunk, before));
    let snapshot = MeshSnapshot::capture(chunk);
    let first = (before * 4) as u32;
    assert_eq!(
        &snapshot.indices[before * 6..],
        &[0, 1, 2, 2, 1, 3].map(|k| first + k)
    );
    // The native front-wall normal (0, 0, -1) in the glTF frame.
    assert!(
        snapshot.normals[before * 4..]
            .iter()
            .all(|n| *n == [0.0, 0.0, 1.0])
    );

    assert!(add_wall(app.world_mut(), (0, 0), (0, 1), &brush).is_err());
    assert!(add_wall(app.world_mut(), (0, 1), (0, 0), &brush).is_err());
    assert_eq!(quads(&app), before + 1);

    collect_document(app.world_mut(), &layout, false)
        .unwrap()
        .validate()
        .unwrap();

    undo(app.world_mut());
    assert_eq!(quads(&app), before);
}

#[test]
fn cells_that_do_not_share_an_edge_get_no_wall() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let uri = app.world().get::<MapTextureList>(layout.root).unwrap().0[0]
        .uri
        .clone();
    let brush = WallBrush { texture_uri: uri };

    assert!(add_wall(app.world_mut(), (0, 0), (1, 1), &brush).is_err());
    assert!(
        add_wall(app.world_mut(), (1, 1), (2, 1), &brush).is_err(),
        "(2, 1) is off the map"
    );
}
