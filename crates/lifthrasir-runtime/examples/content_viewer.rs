//! Standalone content viewer, run from the repository root after asset conversion.
//! `cargo run -p lifthrasir-runtime --example content_viewer -- map ro://maps/prontera/prontera.glb`
//! Other modes: `sprite poring` (monster SPR/ACT stem), `model ro://models/3dmob/guildflag90_1.glb`.
//! Opens a window/audio device only when explicitly run; compile-only validation is safe.

use bevy::{
    asset::{LoadState, RecursiveDependencyLoadState, UntypedAssetId},
    camera::Exposure,
    prelude::*,
};
use bevy_kira_audio::SpatialAudioReceiver;
use lifthrasir_assets::{
    ContentAssetsPlugin,
    catalogs::ContentCatalogPlugin,
    loaders::{RoActAsset, RoSpriteAsset},
    paths,
    source::{AssetConfig, register_ro_asset_source, setup_composite_source_from_config},
};
use lifthrasir_runtime::{
    ContentRuntimePlugins,
    actors::sprites::kinds::SpriteRoot,
    actors::{
        ActorVisualStatus, CharacterDirection, EntitySpriteData, EntitySpriteInfo, Gr2Actor,
        RequestSpriteSpawn,
    },
    audio::ContentAudioListener,
    world::{LifMapData, MapSceneRequest, MapSceneStatus},
};
use std::sync::{Arc, RwLock};

#[derive(Resource)]
enum Selection {
    Map(String),
    Sprite(String),
    Model(String),
}
#[derive(Component)]
struct ViewerContent;
#[derive(Component)]
struct ViewerCamera;
#[derive(Resource)]
struct SpriteLoads {
    sprite: Handle<RoSpriteAsset>,
    action: Handle<RoActAsset>,
}

fn parse_selection(kind: &str, path: &str) -> Result<Selection, &'static str> {
    match kind {
        "map" if path.starts_with("ro://") && path.ends_with(".glb") => {
            Ok(Selection::Map(path.into()))
        }
        "model" if path.starts_with("ro://") && path.ends_with(".glb") => {
            Ok(Selection::Model(path.into()))
        }
        "sprite" if !path.is_empty() => Ok(Selection::Sprite(path.into())),
        _ => Err(
            "map/model require an explicit ro://...glb path; sprite requires a monster SPR/ACT stem",
        ),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [kind, path] = args.as_slice() else {
        return Err(
            "usage: content_viewer map ro://...glb | sprite monster_stem | model ro://...glb"
                .into(),
        );
    };
    let selection = parse_selection(kind, path)?;
    let config: AssetConfig = toml::from_str(&std::fs::read_to_string("assets/loader.toml")?)?;
    let source = setup_composite_source_from_config(&config)?;
    let mut app = App::new();
    register_ro_asset_source(&mut app, Arc::new(RwLock::new(source)));
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: format!("Content viewer: {kind} {path}"),
                    ..default()
                }),
                ..default()
            })
            .disable::<bevy::audio::AudioPlugin>(),
    )
    .add_plugins((
        ContentAssetsPlugin,
        ContentCatalogPlugin,
        ContentRuntimePlugins,
    ))
    .insert_resource(selection)
    .add_systems(Startup, setup)
    .add_systems(Update, (frame_map, report_failure));
    match app.run() {
        AppExit::Success => Ok(()),
        AppExit::Error(_) => Err("content viewer failed; see path-qualified diagnostics".into()),
    }
}

fn setup(mut commands: Commands, selection: Res<Selection>, assets: Res<AssetServer>) {
    commands.spawn((
        Camera3d::default(),
        ViewerCamera,
        Projection::Perspective(PerspectiveProjection {
            far: 10000.0,
            ..default()
        }),
        Transform::from_xyz(0.0, 35.0, 140.0).looking_at(Vec3::new(0.0, 10.0, 0.0), Vec3::Y),
        ContentAudioListener,
        SpatialAudioReceiver,
    ));
    match selection.as_ref() {
        Selection::Map(path) => {
            commands.spawn((ViewerContent, MapSceneRequest { path: path.clone() }));
        }
        Selection::Model(path) => {
            commands.spawn((
                ViewerContent,
                Gr2Actor {
                    model: path.clone(),
                },
                CharacterDirection::default(),
            ));
        }
        Selection::Sprite(stem) => {
            let entity = commands
                .spawn((ViewerContent, SpriteRoot, CharacterDirection::default()))
                .id();
            commands.trigger(RequestSpriteSpawn {
                entity,
                position: Vec3::ZERO,
                sprite_info: EntitySpriteInfo {
                    sprite_data: EntitySpriteData::Mob {
                        sprite_name: stem.clone(),
                    },
                },
            });
            commands.insert_resource(SpriteLoads {
                sprite: assets.load(paths::mob_sprite_path(stem)),
                action: assets.load(paths::mob_action_path(stem)),
            });
        }
    }
    if !matches!(selection.as_ref(), Selection::Map(_)) {
        commands.spawn((
            DirectionalLight {
                illuminance: 20000.0,
                ..default()
            },
            Transform::default().looking_to(Vec3::new(-1.0, -1.0, -1.0), Vec3::Y),
        ));
    }
}

fn frame_map(
    mut commands: Commands,
    maps: Query<&LifMapData, (With<ViewerContent>, Added<LifMapData>)>,
    mut cameras: Query<(&mut Transform, &mut Exposure), With<ViewerCamera>>,
) {
    for data in &maps {
        let width =
            data.altitude.width as f32 * lifthrasir_runtime::world::terrain::RO_UNITS_PER_CELL;
        let height =
            data.altitude.height as f32 * lifthrasir_runtime::world::terrain::RO_UNITS_PER_CELL;
        let center = Vec3::new(width * 0.5, 0.0, -height * 0.5);
        let distance = width.max(height).max(50.0);
        for (mut transform, mut exposure) in &mut cameras {
            *transform = Transform::from_translation(center + Vec3::new(0.0, distance, distance))
                .looking_at(center, Vec3::Y);
            exposure.ev100 = data.meta.exposure_ev100;
        }
        commands.insert_resource(GlobalAmbientLight {
            color: Color::srgb(
                data.meta.ambient_color[0],
                data.meta.ambient_color[1],
                data.meta.ambient_color[2],
            ),
            brightness: data.meta.ambient_brightness,
            affects_lightmapped_meshes: false,
        });
    }
}

fn asset_error(assets: &AssetServer, id: UntypedAssetId) -> Option<String> {
    match assets.load_state(id) {
        LoadState::Failed(error) => Some(error.to_string()),
        _ => match assets.get_recursive_dependency_load_state(id) {
            Some(RecursiveDependencyLoadState::Failed(error)) => Some(error.to_string()),
            _ => None,
        },
    }
}

fn report_failure(
    assets: Res<AssetServer>,
    sprite: Option<Res<SpriteLoads>>,
    maps: Query<&MapSceneStatus, With<ViewerContent>>,
    models: Query<&ActorVisualStatus, With<ViewerContent>>,
    mut exit: MessageWriter<AppExit>,
) {
    let map_error = maps.iter().find_map(|status| {
        if let MapSceneStatus::Failed(error) = status {
            Some(error.clone())
        } else {
            None
        }
    });
    let model_error = models.iter().find_map(|status| {
        if let ActorVisualStatus::Failed(error) = status {
            Some(error.clone())
        } else {
            None
        }
    });
    let sprite_error = sprite.and_then(|sprite| {
        [sprite.sprite.id().untyped(), sprite.action.id().untyped()]
            .into_iter()
            .find_map(|id| {
                asset_error(&assets, id).map(|error| {
                    format!(
                        "{}: {error}",
                        assets
                            .get_path(id)
                            .map_or_else(|| "sprite input".into(), |path| path.to_string())
                    )
                })
            })
    });
    if let Some(error) = map_error.or(model_error).or(sprite_error) {
        error!("Content viewer: {error}");
        exit.write(AppExit::error());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inputs_select_content_without_starting_the_app() {
        assert!(matches!(
            parse_selection("map", "ro://maps/test/test.glb"),
            Ok(Selection::Map(_))
        ));
        assert!(matches!(
            parse_selection("model", "ro://models/test.glb"),
            Ok(Selection::Model(_))
        ));
        assert!(matches!(
            parse_selection("sprite", "poring"),
            Ok(Selection::Sprite(_))
        ));
        assert!(parse_selection("map", "map.gnd").is_err());
        assert!(parse_selection("model", "ro://model.gr2").is_err());
        assert!(parse_selection("sprite", "").is_err());
    }
}
