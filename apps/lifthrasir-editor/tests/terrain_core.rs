//! The terrain core on the loaded fixture: the chunk index, undoable chunk
//! mesh edits, and new chunks for a texture the map does not use yet.

mod common;

use bevy::{gltf::GltfMaterialName, prelude::*};
use lifthrasir_editor::{
    map::{
        document::collect_document,
        layout::{MapLayout, MapTextureList},
        terrain::{MeshSnapshot, TerrainChunks, quad_count, spawn_chunk},
    },
    shell::undo::{MeshChunkEdit, commit, undo},
};
use lifthrasir_tooling::converters::map::document::TextureRef;

const ALL: ([u32; 2], [u32; 2]) = ([0, 0], [u32::MAX, u32::MAX]);

/// Every attribute's bytes plus the index bytes, for exact comparison.
fn mesh_bytes(app: &App, mesh: &Handle<Mesh>) -> Vec<Vec<u8>> {
    let mesh = app.world().resource::<Assets<Mesh>>().get(mesh).unwrap();
    let mut bytes: Vec<Vec<u8>> = mesh
        .attributes()
        .map(|(_, values)| values.get_bytes().to_vec())
        .collect();
    bytes.push(
        mesh.indices()
            .unwrap()
            .iter()
            .flat_map(|i| (i as u32).to_le_bytes())
            .collect(),
    );
    bytes
}

fn first_chunk(app: &App) -> (Entity, Handle<Mesh>) {
    let (min, max) = ALL;
    app.world().resource::<TerrainChunks>().chunks_in(min, max)[0].clone()
}

#[test]
fn the_index_lists_every_terrain_primitive_by_its_derived_chunk() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let doc = collect_document(app.world_mut(), &layout, true).unwrap();
    assert!(!doc.terrain.is_empty());

    let (min, max) = ALL;
    let chunks = app.world().resource::<TerrainChunks>().chunks_in(min, max);
    assert_eq!(chunks.len(), doc.terrain.len());
    for primitive in &doc.terrain {
        let (entity, mesh) = app
            .world()
            .resource::<TerrainChunks>()
            .get(primitive.chunk, &primitive.texture)
            .unwrap_or_else(|| panic!("{:?} {} not indexed", primitive.chunk, primitive.texture));
        let world = app.world();
        assert_eq!(world.get::<Mesh3d>(entity).unwrap().0, mesh);
        assert_eq!(
            world.get::<GltfMaterialName>(entity).unwrap().0,
            primitive.texture
        );
        assert_eq!(
            world.get::<ChildOf>(entity).unwrap().parent(),
            layout.terrain_node.unwrap()
        );
    }
}

#[test]
fn a_chunk_edit_restores_mesh_bytes_and_hides_an_emptied_chunk() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    common::open_fixture(&mut app);
    let (chunk, mesh) = first_chunk(&app);
    let original = mesh_bytes(&app, &mesh);
    let before = MeshSnapshot::capture(app.world().resource::<Assets<Mesh>>().get(&mesh).unwrap());
    let mut after = before.clone();
    after.colors.iter_mut().for_each(|color| color[0] = 0.25);

    commit(
        app.world_mut(),
        Box::new(MeshChunkEdit::new(
            chunk,
            mesh.clone(),
            before.clone(),
            after,
        )),
    );
    assert_ne!(mesh_bytes(&app, &mesh), original);
    undo(app.world_mut());
    assert_eq!(mesh_bytes(&app, &mesh), original);

    commit(
        app.world_mut(),
        Box::new(MeshChunkEdit::new(
            chunk,
            mesh.clone(),
            before,
            MeshSnapshot::default(),
        )),
    );
    let mesh_now = app.world().resource::<Assets<Mesh>>().get(&mesh).unwrap();
    assert_eq!(quad_count(mesh_now), 0);
    assert_eq!(
        app.world().get::<Visibility>(chunk),
        Some(&Visibility::Hidden)
    );
    undo(app.world_mut());
    assert_eq!(mesh_bytes(&app, &mesh), original);
    assert_eq!(
        app.world().get::<Visibility>(chunk),
        Some(&Visibility::Inherited)
    );
}

#[test]
fn a_new_chunk_clones_a_terrain_material_with_only_the_texture_swapped() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout: MapLayout = common::open_fixture(&mut app);
    common::give_chunks_pbr_materials(&mut app);
    let (source, _) = first_chunk(&app);
    let texture = TextureRef {
        name: "dirt".into(),
        uri: "ro://ground/tex/dirt.ktx2".into(),
    };

    let (entity, mesh) = spawn_chunk(app.world_mut(), &layout, [3, 4], &texture).unwrap();

    let world = app.world();
    assert_eq!(
        world.get::<ChildOf>(entity).unwrap().parent(),
        layout.terrain_node.unwrap()
    );
    assert_eq!(world.get::<GltfMaterialName>(entity).unwrap().0, "dirt");
    assert_eq!(world.get::<Visibility>(entity), Some(&Visibility::Hidden));
    assert_eq!(
        world.resource::<TerrainChunks>().get([3, 4], "dirt"),
        Some((entity, mesh.clone()))
    );
    assert_eq!(
        quad_count(world.resource::<Assets<Mesh>>().get(&mesh).unwrap()),
        0
    );
    let materials = world.resource::<Assets<StandardMaterial>>();
    let material_of = |chunk: Entity| {
        let handle = &world
            .get::<MeshMaterial3d<StandardMaterial>>(chunk)
            .unwrap()
            .0;
        materials.get(handle).unwrap().clone()
    };
    let (mut new, old) = (material_of(entity), material_of(source));
    let texture_path = new
        .base_color_texture
        .as_ref()
        .and_then(|t| t.path().cloned());
    assert_eq!(texture_path.unwrap().to_string(), texture.uri);
    new.base_color_texture = old.base_color_texture.clone();
    assert_eq!(format!("{new:?}"), format!("{old:?}"));
    assert!(
        world
            .get::<MapTextureList>(layout.root)
            .unwrap()
            .0
            .iter()
            .all(|known| known.name != "dirt"),
        "spawn_chunk does not touch the texture list"
    );
}
