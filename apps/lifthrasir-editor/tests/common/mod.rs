//! Shared headless helpers for the editor's integration tests.

#![allow(dead_code)]

use bevy::{
    gltf::{Gltf, GltfMaterial, GltfPlugin},
    image::{CompressedImageFormats, ImageLoader},
    mesh::MeshPlugin,
    prelude::*,
    world_serialization::WorldSerializationPlugin,
};
use lifthrasir_assets::source::{
    AssetSource, AssetSourceError, CompositeAssetSource, DataFolderSource, register_ro_asset_source,
};
use lifthrasir_editor::{
    EditorPaths,
    map::{
        MapEditorPlugin,
        layout::{MapLayout, MapTextureList, resolve_layout, texture_list_from_gltf},
        open::{EditorScreen, OpenMapRequest},
    },
    shell::EditorShellPlugin,
};
use lifthrasir_runtime::world::{
    MapEmittersPlugin, MapGltfPlugin, MapScenePlugin, MapSceneRequest, MapSceneStatus,
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

pub const FIXTURE_PATH: &str = "ro://mini_map.glb";
pub const FIXTURE_NAME: &str = "mini_map";
/// Where [`editor_app`] serves the fixture, like a retail map in the pak.
pub const FIXTURE_MAP_PATH: &str = "ro://maps/mini_map/mini_map.glb";

/// The runtime's glb fixtures, written by tooling's
/// `regenerate_game_engine_fixtures`.
pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crates/lifthrasir-runtime/tests/fixtures")
}

/// A windowless app with the runtime's map scene stack and an `ro://` source
/// over the fixtures, registered like production so
/// `SharedCompositeAssetSource` exists.
pub fn runtime_app() -> App {
    let mut source = CompositeAssetSource::new();
    source.add_source(Box::new(DataFolderSource::new(fixtures_dir())));
    finish(app_with_source(source))
}

/// Stands in for the pak: the fixture at its map path, its ground texture,
/// and the prop model and sound file it references (never loaded headless).
struct FixturePak(HashMap<&'static str, Vec<u8>>);

impl FixturePak {
    fn new() -> Self {
        let read = |file: &str| std::fs::read(fixtures_dir().join(file)).unwrap();
        Self(HashMap::from([
            ("maps/mini_map/mini_map.glb", read("mini_map.glb")),
            ("ground/tex/grass01.ktx2", read("ground/tex/grass01.ktx2")),
            ("models/prontera/tree01.glb", read("uv_prop.glb")),
            ("data/wav/effect/water.wav", Vec::new()),
        ]))
    }
}

impl AssetSource for FixturePak {
    fn name(&self) -> &str {
        "FixturePak"
    }
    fn priority(&self) -> u32 {
        1
    }
    fn exists(&self, path: &str) -> bool {
        self.0.contains_key(path)
    }
    fn load(&self, path: &str) -> Result<Vec<u8>, AssetSourceError> {
        self.0
            .get(path)
            .cloned()
            .ok_or_else(|| AssetSourceError::NotFound(path.into()))
    }
    fn list_files(&self) -> Vec<String> {
        self.0.keys().map(|path| path.to_string()).collect()
    }
}

/// The editor without egui: the runtime map stack, the editor shell and map
/// plugins, `ro://` = loose `data_folder` over a fixture pak, and saves going
/// to `data_folder`.
pub fn editor_app(data_folder: &Path) -> App {
    let mut source = CompositeAssetSource::new();
    source.add_source(Box::new(DataFolderSource::new(data_folder)));
    source.add_source(Box::new(FixturePak::new()));
    let mut app = app_with_source(source);
    app.add_plugins((
        bevy::state::app::StatesPlugin,
        bevy::input::InputPlugin,
        EditorShellPlugin,
        MapEditorPlugin,
    ))
    .insert_resource(EditorPaths {
        data_folder: data_folder.to_path_buf(),
    });
    finish(app)
}

/// Opens the fixture through `OpenMapRequest` and updates until editing.
pub fn open_fixture(app: &mut App) -> MapLayout {
    app.world_mut().write_message(OpenMapRequest {
        path: FIXTURE_MAP_PATH.into(),
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    while *app.world().resource::<State<EditorScreen>>().get() != EditorScreen::Editing {
        assert!(Instant::now() < deadline, "the fixture never opened");
        app.update();
        std::thread::sleep(Duration::from_millis(2));
    }
    app.world().resource::<MapLayout>().clone()
}

fn app_with_source(source: CompositeAssetSource) -> App {
    let mut app = App::new();
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
    app
}

/// `GltfPlugin` registers its loader in `finish`, which `update` never calls.
fn finish(mut app: App) -> App {
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
