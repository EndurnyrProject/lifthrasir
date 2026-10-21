//! Painting a tile with a texture the map lacks moves the cell's quad into a
//! new chunk, adds the texture to the map, saves valid, and undoes whole.

mod common;

use bevy::{gltf::GltfMaterialName, prelude::*};
use lifthrasir_editor::{
    map::{
        document::collect_document,
        layout::MapTextureList,
        terrain::{MeshSnapshot, TerrainChunks, ground_quad_at, quad_count},
        tools::paint::{TileBrush, begin_stroke, end_stroke, paint_cell},
    },
    shell::undo::undo,
};

const DIRT: &str = "ro://ground/tex/dirt.ktx2";

fn chunks(app: &App) -> Vec<(Entity, Handle<Mesh>)> {
    app.world()
        .resource::<TerrainChunks>()
        .chunks_in([0, 0], [u32::MAX, u32::MAX])
}

fn snapshot(app: &App, mesh: &Handle<Mesh>) -> MeshSnapshot {
    MeshSnapshot::capture(app.world().resource::<Assets<Mesh>>().get(mesh).unwrap())
}

fn material(app: &App, chunk: Entity) -> StandardMaterial {
    let handle = &app
        .world()
        .get::<MeshMaterial3d<StandardMaterial>>(chunk)
        .unwrap()
        .0;
    app.world()
        .resource::<Assets<StandardMaterial>>()
        .get(handle)
        .unwrap()
        .clone()
}

#[test]
fn painting_a_new_texture_adds_a_chunk_saves_valid_and_undoes() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    common::give_chunks_pbr_materials(&mut app);
    let before = chunks(&app);
    let (source, source_mesh) = before
        .iter()
        .find(|(_, mesh)| {
            let assets = app.world().resource::<Assets<Mesh>>();
            ground_quad_at(assets.get(mesh).unwrap(), (0, 0)).is_some()
        })
        .cloned()
        .unwrap();
    let source_before = snapshot(&app, &source_mesh);
    let textures_before = collect_document(app.world_mut(), &layout, true)
        .unwrap()
        .textures
        .len();

    begin_stroke(app.world_mut());
    paint_cell(app.world_mut(), (0, 0), &TileBrush::new(DIRT.into()));
    end_stroke(app.world_mut());

    let after = chunks(&app);
    assert_eq!(after.len(), before.len() + 1);
    let (dirt, dirt_mesh) = app
        .world()
        .resource::<TerrainChunks>()
        .get([0, 0], "dirt")
        .unwrap();
    assert_eq!(app.world().get::<GltfMaterialName>(dirt).unwrap().0, "dirt");
    assert_eq!(
        app.world().get::<Visibility>(dirt),
        Some(&Visibility::Inherited)
    );
    let assets = app.world().resource::<Assets<Mesh>>();
    assert_eq!(quad_count(assets.get(&dirt_mesh).unwrap()), 1);
    assert_eq!(
        ground_quad_at(assets.get(&dirt_mesh).unwrap(), (0, 0)),
        Some(0)
    );
    assert_eq!(
        ground_quad_at(assets.get(&source_mesh).unwrap(), (0, 0)),
        None
    );

    let (mut new, old) = (material(&app, dirt), material(&app, source));
    let path = new
        .base_color_texture
        .as_ref()
        .and_then(|t| t.path().cloned());
    assert_eq!(path.unwrap().to_string(), DIRT);
    new.base_color_texture = old.base_color_texture.clone();
    assert_eq!(format!("{new:?}"), format!("{old:?}"));

    let doc = collect_document(app.world_mut(), &layout, false).unwrap();
    assert_eq!(doc.textures.len(), textures_before + 1);
    assert!(
        doc.textures
            .iter()
            .any(|t| t.name == "dirt" && t.uri == DIRT)
    );
    assert!(doc.terrain.iter().any(|p| p.texture == "dirt"));
    doc.validate().unwrap();

    undo(app.world_mut());
    assert_eq!(snapshot(&app, &source_mesh), source_before);
    assert!(snapshot(&app, &dirt_mesh).is_empty());
    assert_eq!(
        app.world().get::<Visibility>(dirt),
        Some(&Visibility::Hidden)
    );
    assert!(
        app.world()
            .get::<MapTextureList>(layout.root)
            .unwrap()
            .0
            .iter()
            .any(|t| t.uri == DIRT),
        "the texture list append is not undone"
    );
}

#[test]
fn repainting_a_cell_with_its_own_texture_only_turns_its_uvs() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let texture = app.world().get::<MapTextureList>(layout.root).unwrap().0[0].clone();
    let count = chunks(&app).len();
    let brush = TileBrush {
        texture_uri: texture.uri,
        rotation: 1,
        flip: false,
    };

    begin_stroke(app.world_mut());
    paint_cell(app.world_mut(), (0, 0), &brush);
    end_stroke(app.world_mut());

    assert_eq!(chunks(&app).len(), count, "no chunk is added");
    let (_, mesh) = app
        .world()
        .resource::<TerrainChunks>()
        .get([0, 0], &texture.name)
        .unwrap();
    let snapshot = snapshot(&app, &mesh);
    let quad = ground_quad_at(
        app.world().resource::<Assets<Mesh>>().get(&mesh).unwrap(),
        (0, 0),
    )
    .unwrap();
    assert_eq!(
        &snapshot.uvs[quad * 4..quad * 4 + 4],
        &lifthrasir_editor::map::tools::paint::tile_uvs(1, false)
    );
}
