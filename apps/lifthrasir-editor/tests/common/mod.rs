//! Shared headless helpers for the editor's integration tests.

#![allow(dead_code)]

use bevy::{
    gltf::{Gltf, GltfMaterial, GltfPlugin},
    image::{CompressedImageFormats, ImageLoader},
    mesh::MeshPlugin,
    prelude::*,
    world_serialization::WorldSerializationPlugin,
};
use lifthrasir_assets::source::{CompositeAssetSource, DataFolderSource, register_ro_asset_source};
use lifthrasir_editor::map::layout::{
    MapLayout, MapTextureList, resolve_layout, texture_list_from_gltf,
};
use lifthrasir_runtime::world::{
    MapEmittersPlugin, MapGltfPlugin, MapScenePlugin, MapSceneRequest, MapSceneStatus,
};
use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

pub const FIXTURE_PATH: &str = "ro://mini_map.glb";
pub const FIXTURE_NAME: &str = "mini_map";

/// The runtime's glb fixtures, written by tooling's
/// `regenerate_game_engine_fixtures`.
pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crates/lifthrasir-runtime/tests/fixtures")
}

/// A windowless app with the runtime's map scene stack and an `ro://` source
/// over the fixtures, registered like production so
/// `SharedCompositeAssetSource` exists.
pub fn runtime_app() -> App {
    let mut app = App::new();
    let mut source = CompositeAssetSource::new();
    source.add_source(Box::new(DataFolderSource::new(fixtures_dir())));
    register_ro_asset_source(&mut app, Arc::new(RwLock::new(source)));
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin::default(),
        ImagePlugin::default(),
        MeshPlugin,
        WorldSerializationPlugin,
        GltfPlugin::default(),
        MapGltfPlugin,
        MapScenePlugin,
        MapEmittersPlugin,
    ))
    .init_asset::<StandardMaterial>();
    app.register_asset_loader(ImageLoader::new(CompressedImageFormats::NONE));
    // `GltfPlugin` registers its loader in `finish`, which `update` never calls.
    app.finish();
    app.cleanup();
    app
}

/// Updates until `root` is `Ready` or `Failed`, panicking after 10 seconds.
pub fn settle_scene(app: &mut App, root: Entity) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        if matches!(
            app.world().get::<MapSceneStatus>(root),
            Some(MapSceneStatus::Ready | MapSceneStatus::Failed(_))
        ) {
            return;
        }
        assert!(Instant::now() < deadline, "scene request never settled");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Loads `mini_map.glb` as a map scene, seeds its `MapTextureList` from the
/// `Gltf` asset and resolves the layout.
pub fn load_fixture(app: &mut App) -> (Entity, MapLayout) {
    let gltf: Handle<Gltf> = app.world().resource::<AssetServer>().load(FIXTURE_PATH);
    let root = app
        .world_mut()
        .spawn(MapSceneRequest {
            path: FIXTURE_PATH.into(),
        })
        .id();
    settle_scene(app, root);
    assert_eq!(
        app.world().get::<MapSceneStatus>(root),
        Some(&MapSceneStatus::Ready)
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while !app.world().resource::<Assets<Gltf>>().contains(&gltf) {
        assert!(Instant::now() < deadline, "fixture Gltf never loaded");
        app.update();
        std::thread::sleep(Duration::from_millis(2));
    }
    let textures = {
        let world = app.world();
        texture_list_from_gltf(
            world.resource::<Assets<Gltf>>().get(&gltf).unwrap(),
            world.resource::<Assets<GltfMaterial>>(),
        )
        .unwrap()
    };
    app.world_mut()
        .entity_mut(root)
        .insert(MapTextureList(textures));
    let layout = resolve_layout(app.world(), root, FIXTURE_NAME).unwrap();
    (root, layout)
}
