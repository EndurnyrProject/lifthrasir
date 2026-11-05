//! Effect screenshot capture. Spawns one effect once, beside a character-sized
//! box on a ground plane, saves window screenshots at the given times (seconds
//! after the effect finished loading), then exits. Opens a window, so visual
//! runs require user approval. Run from the repository root:
//! `cargo run -p lifthrasir-runtime --example fx_capture -- ro://effects/bash.strfx.ron /tmp/bash 0.1,0.3,0.5`
//! writes `/tmp/bash_00.png`, `/tmp/bash_01.png`, ... The window is 480x480
//! logical; screenshots are in physical pixels.
//!
//! `shader:<key>` instead of a path plays the `shader_fx` catalog entry `key`
//! at the box, with three hits; traveling entries fly in from the left.

use bevy::asset::{LoadState, RecursiveDependencyLoadState};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use lifthrasir_assets::catalogs::shader_fx::ShaderFxCatalog;
use lifthrasir_assets::effects::LoadedEffectAsset;
use lifthrasir_assets::source::{
    AssetConfig, register_ro_asset_source, setup_composite_source_from_config,
};
use lifthrasir_assets::{ContentAssetsPlugin, ContentCatalogPlugin};
use lifthrasir_runtime::effects::events::PlayProceduralVfx;
use lifthrasir_runtime::effects::{EffectAnchor, spawn_effect};
use lifthrasir_runtime::{EffectsPlaybackPlugin, RuntimeCorePlugin};
use std::sync::{Arc, RwLock};

/// Character stand-in: a thin slab about the size of a player sprite, so effects
/// drawn at the anchor are not hidden behind it as they would be by a solid box.
const BODY_SIZE: Vec3 = Vec3::new(7.0, 20.0, 0.2);

/// Seconds to let the window and renderer settle before spawning the effect.
const WARMUP_SECS: f32 = 1.0;

/// Heights above the feet that shader effects anchor at, mirroring the client's
/// `VFX_BODY_HEIGHT` and, for `ground` entries, `VFX_CENTER_HEIGHT`
/// (`lifthrasir-client` `domain/effects/triggers.rs`).
const SHADER_BODY_LIFT: f32 = 9.0;
const SHADER_GROUND_LIFT: f32 = 2.0;

/// Horizontal offset of the caster traveling shader effects launch from.
const SHADER_SOURCE_X: f32 = -40.0;

/// Hit count passed to shader effects, so `per_hit` bolts launch several.
const SHADER_HITS: u32 = 3;

#[derive(Resource)]
struct CaptureRequest {
    path: String,
    out: String,
    times: Vec<f32>,
}

/// What the capture plays: an STR/strfx asset or a `shader_fx` catalog key.
enum Subject {
    Effect(Handle<LoadedEffectAsset>),
    Shader(String),
}

#[derive(Resource)]
struct CaptureState {
    subject: Subject,
    started: Option<f32>,
    next: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    let [path, out, times] = arguments.as_slice() else {
        return Err(
            "usage: fx_capture (ro://path/effect.strfx.ron | shader:KEY) OUT_PREFIX T1,T2,..."
                .into(),
        );
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
        ContentCatalogPlugin,
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
    let subject = match request.path.strip_prefix("shader:") {
        Some(key) => Subject::Shader(key.to_string()),
        None => Subject::Effect(assets.load(request.path.clone())),
    };
    commands.insert_resource(CaptureState {
        subject,
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
    (shader_fx, mut vfx): (
        Option<Res<ShaderFxCatalog>>,
        MessageWriter<PlayProceduralVfx>,
    ),
    mut exit: MessageWriter<AppExit>,
) {
    let now = time.elapsed_secs();
    let Some(started) = state.started else {
        if now <= WARMUP_SECS {
            return;
        }
        let started = match &state.subject {
            Subject::Effect(handle) => start_effect(&mut commands, &assets, &request.path, handle),
            Subject::Shader(key) => start_shader(shader_fx.as_deref(), key, &mut vfx),
        };
        match started {
            Ok(true) => state.started = Some(now),
            Ok(false) => {}
            Err(message) => {
                error!("{message}");
                exit.write(AppExit::error());
            }
        }
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

/// Spawns the effect once it and its textures are loaded. `Ok(false)` while
/// still loading.
fn start_effect(
    commands: &mut Commands,
    assets: &AssetServer,
    path: &str,
    handle: &Handle<LoadedEffectAsset>,
) -> Result<bool, String> {
    if let LoadState::Failed(error) = assets.load_state(handle) {
        return Err(format!("Failed to load {path}: {error}"));
    }
    match assets.get_recursive_dependency_load_state(handle) {
        Some(RecursiveDependencyLoadState::Failed(error)) => {
            Err(format!("Failed to load {path} dependencies: {error}"))
        }
        Some(RecursiveDependencyLoadState::Loaded) => {
            spawn_effect(
                commands,
                handle.clone(),
                EffectAnchor::Position(Vec3::ZERO),
                false,
                Color::WHITE,
                None,
            );
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// Plays a `shader_fx` entry once the effect catalog has loaded. `Ok(false)`
/// while the catalog is still loading.
fn start_shader(
    catalog: Option<&ShaderFxCatalog>,
    key: &str,
    vfx: &mut MessageWriter<PlayProceduralVfx>,
) -> Result<bool, String> {
    let Some(catalog) = catalog else {
        return Ok(false);
    };
    let Some(entry) = catalog.get(key) else {
        return Err(format!(
            "no shader_fx entry {key:?} in ro://ron/effects.ron"
        ));
    };
    let lift = if entry.ground {
        SHADER_GROUND_LIFT
    } else {
        SHADER_BODY_LIFT
    };
    vfx.write(PlayProceduralVfx {
        key: key.to_string(),
        position: Vec3::new(0.0, lift, 0.0),
        source: Some(Vec3::new(SHADER_SOURCE_X, lift, 0.0)),
        hits: SHADER_HITS,
        sound: None,
        color: Color::WHITE,
    });
    Ok(true)
}
