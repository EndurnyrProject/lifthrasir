//! Offline effect viewer. Run from the repository root with an explicit content path:
//! `cargo run -p lifthrasir-runtime --example effect_preview -- ro://effects/bash.strfx.ron`.
//! Uses assets/loader.toml and shared crates only. Visual runs require user approval.
//! Deliberately installs the silent core/effects subset, not the audio-owning runtime group.

use bevy::asset::{LoadState, RecursiveDependencyLoadState};
use bevy::prelude::*;
use lifthrasir_assets::ContentAssetsPlugin;
use lifthrasir_assets::effects::LoadedEffectAsset;
use lifthrasir_assets::source::{
    AssetConfig, register_ro_asset_source, setup_composite_source_from_config,
};
use lifthrasir_runtime::effects::{EffectAnchor, spawn_effect};
use lifthrasir_runtime::{EffectsPlaybackPlugin, RuntimeCorePlugin};
use std::sync::{Arc, RwLock};

#[derive(Resource)]
struct PreviewPath(String);

#[derive(Resource)]
struct PreviewHandle(Handle<LoadedEffectAsset>);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    let [path] = arguments.as_slice() else {
        return Err("usage: effect_preview ro://path/to/effect.strfx.ron (or .str)".into());
    };
    if !path.starts_with("ro://") || !(path.ends_with(".strfx.ron") || path.ends_with(".str")) {
        return Err("effect path must be an explicit ro:// .strfx.ron or .str path".into());
    }
    let config: AssetConfig = toml::from_str(&std::fs::read_to_string("assets/loader.toml")?)?;
    let source = setup_composite_source_from_config(&config)?;
    let mut app = App::new();
    register_ro_asset_source(&mut app, Arc::new(RwLock::new(source)));
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: format!("Effect preview: {path}"),
            ..default()
        }),
        ..default()
    }))
    .add_plugins((
        ContentAssetsPlugin,
        RuntimeCorePlugin,
        EffectsPlaybackPlugin,
    ))
    .insert_resource(PreviewPath(path.clone()))
    .insert_resource(ClearColor(Color::BLACK))
    .add_systems(Startup, setup)
    .add_systems(Update, report_failure);
    match app.run() {
        AppExit::Success => Ok(()),
        AppExit::Error(_) => {
            Err("effect preview failed; see the path-qualified error above".into())
        }
    }
}

fn setup(mut commands: Commands, assets: Res<AssetServer>, path: Res<PreviewPath>) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 20.0, 180.0).looking_at(Vec3::new(0.0, 10.0, 0.0), Vec3::Y),
    ));
    let handle = assets.load(path.0.clone());
    spawn_effect(
        &mut commands,
        handle.clone(),
        EffectAnchor::Position(Vec3::ZERO),
        true,
        Color::WHITE,
        None,
    );
    commands.insert_resource(PreviewHandle(handle));
}

fn report_failure(
    assets: Res<AssetServer>,
    path: Res<PreviewPath>,
    handle: Res<PreviewHandle>,
    mut exit: MessageWriter<AppExit>,
) {
    let failure = match assets.load_state(&handle.0) {
        LoadState::Failed(error) => Some(error),
        _ => match assets.get_recursive_dependency_load_state(&handle.0) {
            Some(RecursiveDependencyLoadState::Failed(error)) => Some(error),
            _ => None,
        },
    };
    if let Some(error) = failure {
        error!("Failed to preview {}: {error}", path.0);
        exit.write(AppExit::error());
    }
}
