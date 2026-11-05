//! Effect screenshot capture. Spawns one effect once, beside a character-sized
//! box on a ground plane, saves window screenshots at the given times (seconds
//! after the effect finished loading), then exits. Opens a window, so visual
//! runs require user approval. Run from the repository root:
//! `cargo run -p lifthrasir-runtime --example fx_capture -- ro://effects/bash.strfx.ron /tmp/bash 0.1,0.3,0.5`
//! writes `/tmp/bash_00.png`, `/tmp/bash_01.png`, ... The window is 480x480
//! logical; screenshots are in physical pixels.

use bevy::asset::{LoadState, RecursiveDependencyLoadState};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use lifthrasir_assets::ContentAssetsPlugin;
use lifthrasir_assets::effects::LoadedEffectAsset;
use lifthrasir_assets::source::{
    AssetConfig, register_ro_asset_source, setup_composite_source_from_config,
};
use lifthrasir_runtime::effects::{EffectAnchor, spawn_effect};
use lifthrasir_runtime::{EffectsPlaybackPlugin, RuntimeCorePlugin};
use std::sync::{Arc, RwLock};

/// Character stand-in: about the height of a player sprite in world units.
const BODY_SIZE: Vec3 = Vec3::new(7.0, 20.0, 3.0);

/// Seconds to let the window and renderer settle before spawning the effect.
const WARMUP_SECS: f32 = 1.0;

#[derive(Resource)]
struct CaptureRequest {
    path: String,
    out: String,
    times: Vec<f32>,
}

#[derive(Resource)]
struct CaptureState {
    handle: Handle<LoadedEffectAsset>,
    started: Option<f32>,
    next: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    let [path, out, times] = arguments.as_slice() else {
        return Err("usage: fx_capture ro://path/effect.strfx.ron OUT_PREFIX T1,T2,...".into());
    };
    let times = times
        .split(',')
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()?;
    if times.is_empty() || times.windows(2).any(|pair| pair[0] > pair[1]) {
        return Err("capture times must be ascending seconds, e.g. 0.1,0.3,0.5".into());
    }
    let config: AssetConfig = toml::from_str(&std::fs::read_to_string("assets/loader.toml")?)?;
    let source = setup_composite_source_from_config(&config)?;
    let mut app = App::new();
    register_ro_asset_source(&mut app, Arc::new(RwLock::new(source)));
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: format!("Effect capture: {path}"),
                    resolution: (480, 480).into(),
                    ..default()
                }),
                ..default()
            })
            .disable::<bevy::audio::AudioPlugin>(),
    )
    .add_plugins((
        ContentAssetsPlugin,
        RuntimeCorePlugin,
        EffectsPlaybackPlugin,
    ))
    .insert_resource(ClearColor(Color::srgb(0.18, 0.2, 0.18)))
    .insert_resource(CaptureRequest {
        path: path.clone(),
        out: out.clone(),
        times,
    })
    .add_systems(Startup, setup)
    .add_systems(Update, capture);
    match app.run() {
        AppExit::Success => Ok(()),
        AppExit::Error(_) => Err("effect capture failed; see the error above".into()),
    }
}

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    request: Res<CaptureRequest>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 30.0, 70.0).looking_at(Vec3::new(0.0, 10.0, 0.0), Vec3::Y),
    ));
    let mut unlit = |color: Color| {
        materials.add(StandardMaterial {
            base_color: color,
            unlit: true,
            ..default()
        })
    };
    let body_material = unlit(Color::srgb(0.35, 0.3, 0.45));
    let ground_material = unlit(Color::srgb(0.25, 0.3, 0.22));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::from_size(BODY_SIZE))),
        MeshMaterial3d(body_material),
        Transform::from_xyz(0.0, BODY_SIZE.y / 2.0, 0.0),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(60.0, 60.0))),
        MeshMaterial3d(ground_material),
    ));
    commands.insert_resource(CaptureState {
        handle: assets.load(request.path.clone()),
        started: None,
        next: 0,
    });
}

fn capture(
    mut commands: Commands,
    assets: Res<AssetServer>,
    time: Res<Time>,
    request: Res<CaptureRequest>,
    mut state: ResMut<CaptureState>,
    mut exit: MessageWriter<AppExit>,
) {
    let now = time.elapsed_secs();
    let Some(started) = state.started else {
        start_when_loaded(
            &mut commands,
            &assets,
            now,
            &request.path,
            &mut state,
            &mut exit,
        );
        return;
    };
    let elapsed = now - started;
    let Some(&at) = request.times.get(state.next) else {
        if elapsed > request.times[request.times.len() - 1] + 0.5 {
            exit.write(AppExit::Success);
        }
        return;
    };
    if elapsed < at {
        return;
    }
    let path = format!("{}_{:02}.png", request.out, state.next);
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
    state.next += 1;
}

fn start_when_loaded(
    commands: &mut Commands,
    assets: &AssetServer,
    now: f32,
    path: &str,
    state: &mut CaptureState,
    exit: &mut MessageWriter<AppExit>,
) {
    if let LoadState::Failed(error) = assets.load_state(&state.handle) {
        error!("Failed to load {path}: {error}");
        exit.write(AppExit::error());
        return;
    }
    match assets.get_recursive_dependency_load_state(&state.handle) {
        Some(RecursiveDependencyLoadState::Failed(error)) => {
            error!("Failed to load {path} dependencies: {error}");
            exit.write(AppExit::error());
        }
        Some(RecursiveDependencyLoadState::Loaded) if now > WARMUP_SECS => {
            spawn_effect(
                commands,
                state.handle.clone(),
                EffectAnchor::Position(Vec3::ZERO),
                false,
                Color::WHITE,
                None,
            );
            state.started = Some(now);
        }
        _ => {}
    }
}
