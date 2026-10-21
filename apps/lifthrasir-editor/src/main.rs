//! `cargo run -p lifthrasir-editor`, from the repository root.
//!
//! Composes the shared content stack like the runtime's `content_viewer`
//! example, then adds egui, mesh picking and the editor plugins.

use bevy::{
    gltf::GltfPlugin, image::ImageSamplerDescriptor, picking::mesh_picking::MeshPickingPlugin,
    prelude::*,
};
use bevy_egui::EguiPlugin;
use lifthrasir_assets::{
    ContentAssetsPlugin,
    catalogs::ContentCatalogPlugin,
    source::{AssetConfig, register_ro_asset_source, setup_composite_source_from_config},
};
use lifthrasir_editor::{EditorPaths, map::MapEditorPlugin, shell::EditorShellPlugin};
use lifthrasir_runtime::ContentRuntimePlugins;
use std::sync::{Arc, RwLock};

const LOADER_CONFIG: &str = "assets/loader.toml";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(LOADER_CONFIG)
        .map_err(|error| format!("reading {LOADER_CONFIG}: {error}"))?;
    let config: AssetConfig =
        toml::from_str(&text).map_err(|error| format!("parsing {LOADER_CONFIG}: {error}"))?;
    let source = setup_composite_source_from_config(&config)?;

    let mut app = App::new();
    register_ro_asset_source(&mut app, Arc::new(RwLock::new(source)));

    let mut gltf_sampler = ImageSamplerDescriptor::linear();
    gltf_sampler.set_anisotropic_filter(16);

    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Lifthrasir Editor".into(),
                    ..default()
                }),
                // Quit goes through the editor's unsaved-changes guard.
                close_when_requested: false,
                ..default()
            })
            .set(GltfPlugin {
                default_sampler: gltf_sampler,
                ..default()
            })
            // Same benign hanabi bind-group-layout race the client silences.
            .set(bevy::log::LogPlugin {
                filter: format!("{},bevy_hanabi::render=off", bevy::log::DEFAULT_FILTER),
                ..default()
            })
            .disable::<bevy::audio::AudioPlugin>(),
    )
    .add_plugins((
        ContentAssetsPlugin,
        ContentCatalogPlugin,
        ContentRuntimePlugins,
        MeshPickingPlugin,
        EguiPlugin::default(),
        EditorShellPlugin,
        MapEditorPlugin,
    ))
    .insert_resource(EditorPaths::from_config(&config));

    match app.run() {
        AppExit::Success => Ok(()),
        AppExit::Error(code) => Err(format!("editor exited with code {code}").into()),
    }
}
