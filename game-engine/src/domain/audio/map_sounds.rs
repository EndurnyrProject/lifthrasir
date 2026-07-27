use crate::core::GameState;
use crate::domain::entities::markers::LocalPlayer;
use crate::domain::world::components::MapLoader;
#[cfg(feature = "map-gltf")]
use crate::domain::world::gltf_map::LifAudioEmitter;
use crate::infrastructure::assets::loaders::{RoGroundAsset, RoWorldAsset};
use crate::infrastructure::ro_formats::RswObject;
use crate::utils::{get_map_dimensions_from_ground, rsw_position_to_bevy};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use bevy_kira_audio::prelude::AudioControl;
use bevy_kira_audio::{AudioChannel, AudioInstance, AudioSource, AudioTween, PlaybackState};
use std::time::Duration;

use super::resources::{AmbienceChannel, AudioSettings};
use super::systems::amplitude_to_decibels;

#[derive(Component)]
pub struct MapSound;

#[derive(Component)]
pub struct MapSoundsSpawned;

pub enum MapSoundState {
    Playing,
    Silent { timer: Timer },
}

#[derive(Component)]
pub struct MapSoundSource {
    pub handle_path: String,
    pub base_volume: f32,
    pub range: f32,
    pub cycle: Duration,
    pub instance: Option<Handle<AudioInstance>>,
    pub state: MapSoundState,
}

pub fn map_sound_path(name: &str) -> String {
    format!("ro://data/wav/{}", name.replace('\\', "/"))
}

/// One map sound source at an already world-space position. Both the RSW path
/// (which converts the RSW coordinates first) and the glb path (whose emitter
/// nodes carry the final world transform) spawn this.
fn map_sound_bundle(
    translation: Vec3,
    handle_path: String,
    base_volume: f32,
    range: f32,
    cycle: f32,
) -> impl Bundle {
    (
        MapSound,
        MapSoundSource {
            handle_path,
            base_volume,
            range,
            cycle: Duration::from_secs_f32(cycle),
            instance: None,
            state: MapSoundState::Silent {
                timer: Timer::new(Duration::ZERO, TimerMode::Once),
            },
        },
        Transform::from_translation(translation),
    )
}

#[auto_add_system(plugin = crate::domain::audio::plugin::AudioPlugin, schedule = Update)]
pub fn spawn_map_sounds(
    mut commands: Commands,
    world_assets: Res<Assets<RoWorldAsset>>,
    ground_assets: Res<Assets<RoGroundAsset>>,
    query: Query<(Entity, &MapLoader), Without<MapSoundsSpawned>>,
) {
    for (entity, map_loader) in query.iter() {
        let Some(world_handle) = &map_loader.world else {
            continue;
        };

        let Some(world_asset) = world_assets.get(world_handle) else {
            continue;
        };

        let Some(ground_asset) = ground_assets.get(&map_loader.ground) else {
            continue;
        };

        let (map_width, map_height) = get_map_dimensions_from_ground(&ground_asset.ground);

        let mut spawned = 0;

        for obj in &world_asset.world.objects {
            let RswObject::Sound(sound) = obj else {
                continue;
            };

            if sound.range <= 0.0 || sound.wav_file.is_empty() {
                debug!(
                    "Skipping map sound '{}' (range={}, wav_file='{}')",
                    sound.name, sound.range, sound.wav_file
                );
                continue;
            }

            commands.spawn(map_sound_bundle(
                rsw_position_to_bevy(sound.position, map_width, map_height),
                map_sound_path(&sound.wav_file),
                sound.volume,
                sound.range,
                sound.cycle,
            ));

            spawned += 1;
        }

        commands.entity(entity).insert(MapSoundsSpawned);

        debug!("Spawned {} map sounds", spawned);
    }
}

/// Marks a glb audio-emitter node whose [`MapSound`] has been spawned. The node
/// is a scene child of the `MapScoped` loader entity, so the marker dies with
/// the map and the next one's emitters spawn afresh -- exactly how
/// [`MapSoundsSpawned`] behaves on the RSW path.
#[cfg(feature = "map-gltf")]
#[derive(Component)]
pub struct GltfMapSoundSpawned;

/// The glb counterpart of [`spawn_map_sounds`]: emitter nodes carry their final
/// world position in `GlobalTransform`, so this must run after transform
/// propagation ([`GltfMapPlugin`] schedules it there).
///
/// Rangeless and fileless sounds never reach a glb -- the converter drops them
/// while emitting nodes, mirroring the guard on the RSW path.
///
/// [`GltfMapPlugin`]: crate::domain::world::GltfMapPlugin
#[cfg(feature = "map-gltf")]
pub fn spawn_gltf_map_sounds(
    mut commands: Commands,
    emitters: Query<(Entity, &LifAudioEmitter, &GlobalTransform), Without<GltfMapSoundSpawned>>,
) {
    for (entity, emitter, transform) in emitters.iter() {
        let audio = &emitter.0;

        commands.spawn(map_sound_bundle(
            transform.translation(),
            audio.file.clone(),
            audio.volume,
            audio.range,
            audio.cycle,
        ));
        commands.entity(entity).insert(GltfMapSoundSpawned);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SoundCycleInput {
    Playing {
        finished: bool,
    },
    Silent {
        timer_finished: bool,
        in_range: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SoundAction {
    Play,
    BeginSilence,
    None,
}

fn next_sound_action(input: SoundCycleInput) -> SoundAction {
    match input {
        SoundCycleInput::Playing { finished: true } => SoundAction::BeginSilence,
        SoundCycleInput::Playing { finished: false } => SoundAction::None,
        SoundCycleInput::Silent {
            timer_finished: true,
            in_range: true,
        } => SoundAction::Play,
        SoundCycleInput::Silent { .. } => SoundAction::None,
    }
}

fn distance_gain(dist: f32, range: f32) -> f32 {
    if range <= 0.0 {
        return 0.0;
    }
    (1.0 - dist / range).clamp(0.0, 1.0)
}

#[auto_add_system(plugin = crate::domain::audio::plugin::AudioPlugin, schedule = Update)]
pub fn drive_map_sound_cycle(
    mut sources: Query<(&mut MapSoundSource, &GlobalTransform)>,
    listener: Query<&GlobalTransform, With<LocalPlayer>>,
    ambience_channel: Res<AudioChannel<AmbienceChannel>>,
    audio_instances: Res<Assets<AudioInstance>>,
    asset_server: Res<AssetServer>,
    time: Res<Time>,
) {
    let Ok(listener_transform) = listener.single() else {
        return;
    };
    let listener_pos = listener_transform.translation();

    for (mut src, transform) in sources.iter_mut() {
        let in_range = listener_pos.distance(transform.translation()) <= src.range;

        let input = match &mut src.state {
            MapSoundState::Playing => {
                let finished = match &src.instance {
                    Some(handle) => {
                        audio_instances.get(handle).map(|i| i.state())
                            == Some(PlaybackState::Stopped)
                    }
                    None => true,
                };
                SoundCycleInput::Playing { finished }
            }
            MapSoundState::Silent { timer } => {
                timer.tick(time.delta());
                SoundCycleInput::Silent {
                    timer_finished: timer.is_finished(),
                    in_range,
                }
            }
        };

        match next_sound_action(input) {
            SoundAction::Play => {
                let source: Handle<AudioSource> = asset_server.load(&src.handle_path);
                let handle = ambience_channel.play(source).handle();
                src.instance = Some(handle);
                src.state = MapSoundState::Playing;
            }
            SoundAction::BeginSilence => {
                src.state = MapSoundState::Silent {
                    timer: Timer::new(src.cycle, TimerMode::Once),
                };
            }
            SoundAction::None => {}
        }
    }
}

#[derive(Resource)]
#[auto_init_resource(plugin = crate::domain::audio::plugin::AudioPlugin)]
pub struct MapSoundUpdateTimer(Timer);

impl Default for MapSoundUpdateTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(0.1, TimerMode::Repeating))
    }
}

#[auto_add_system(plugin = crate::domain::audio::plugin::AudioPlugin, schedule = Update)]
pub fn update_map_sound_volume(
    mut throttle: ResMut<MapSoundUpdateTimer>,
    time: Res<Time>,
    mut sources: Query<(&mut MapSoundSource, &GlobalTransform)>,
    listener: Query<&GlobalTransform, With<LocalPlayer>>,
    audio_settings: Res<AudioSettings>,
    mut audio_instances: ResMut<Assets<AudioInstance>>,
) {
    if !throttle.0.tick(time.delta()).just_finished() {
        return;
    }

    let Ok(listener_transform) = listener.single() else {
        return;
    };
    let listener_pos = listener_transform.translation();

    let tween = AudioTween::linear(Duration::from_secs_f32(0.1));

    for (mut src, transform) in sources.iter_mut() {
        if !matches!(src.state, MapSoundState::Playing) {
            continue;
        }

        let Some(handle) = src.instance.clone() else {
            continue;
        };

        let Some(mut instance) = audio_instances.get_mut(&handle) else {
            continue;
        };

        let dist = listener_pos.distance(transform.translation());

        if dist > src.range {
            instance.stop(tween.clone());
            src.state = MapSoundState::Silent {
                timer: Timer::new(src.cycle, TimerMode::Once),
            };
            continue;
        }

        let gain = distance_gain(dist, src.range);
        let vol = src.base_volume * gain * audio_settings.effective_ambience_volume();
        instance.set_decibels(amplitude_to_decibels(vol), tween.clone());
    }
}

/// Despawn the map's sound-source entities on map exit.
///
/// The `MapSound` sources are intentionally not `MapScoped`, so they need this
/// dedicated despawn. The `MapSoundsSpawned` spawn-once marker lives on the
/// `MapLoader` entity, which *is* `MapScoped` and gets despawned by
/// `despawn_map_scoped` in this same schedule — so the marker dies with it and
/// the next map's fresh loader respawns sounds. Removing it here would just race
/// that despawn and panic on a recycled entity.
#[auto_add_system(plugin = crate::domain::audio::plugin::AudioPlugin, schedule = OnExit(GameState::InGame))]
pub fn teardown_map_sounds(
    mut commands: Commands,
    sources: Query<(Entity, &MapSoundSource), With<MapSound>>,
    mut audio_instances: ResMut<Assets<AudioInstance>>,
) {
    let tween = AudioTween::linear(Duration::from_secs_f32(0.1));

    for (entity, src) in sources.iter() {
        if let Some(handle) = &src.instance
            && let Some(mut instance) = audio_instances.get_mut(handle)
        {
            instance.stop(tween.clone());
        }
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_sound_path_normalizes_backslashes_and_prefixes() {
        assert_eq!(map_sound_path("foo\\bar.wav"), "ro://data/wav/foo/bar.wav");
    }

    #[test]
    fn playing_with_finished_instance_begins_silence() {
        assert_eq!(
            next_sound_action(SoundCycleInput::Playing { finished: true }),
            SoundAction::BeginSilence
        );
    }

    #[test]
    fn playing_with_live_instance_does_nothing() {
        assert_eq!(
            next_sound_action(SoundCycleInput::Playing { finished: false }),
            SoundAction::None
        );
    }

    #[test]
    fn silent_finished_in_range_plays() {
        assert_eq!(
            next_sound_action(SoundCycleInput::Silent {
                timer_finished: true,
                in_range: true,
            }),
            SoundAction::Play
        );
    }

    #[test]
    fn silent_finished_out_of_range_stays_silent() {
        assert_eq!(
            next_sound_action(SoundCycleInput::Silent {
                timer_finished: true,
                in_range: false,
            }),
            SoundAction::None
        );
    }

    #[test]
    fn silent_not_finished_does_nothing() {
        assert_eq!(
            next_sound_action(SoundCycleInput::Silent {
                timer_finished: false,
                in_range: true,
            }),
            SoundAction::None
        );
    }

    #[test]
    fn distance_gain_is_full_at_zero_distance() {
        assert_eq!(distance_gain(0.0, 50.0), 1.0);
    }

    #[test]
    fn distance_gain_is_half_at_mid_range() {
        assert!((distance_gain(25.0, 50.0) - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn distance_gain_is_zero_at_and_beyond_range() {
        assert_eq!(distance_gain(50.0, 50.0), 0.0);
        assert_eq!(distance_gain(75.0, 50.0), 0.0);
    }

    #[test]
    fn distance_gain_guards_non_positive_range() {
        assert_eq!(distance_gain(0.0, 0.0), 0.0);
        assert_eq!(distance_gain(5.0, -10.0), 0.0);
    }

    #[test]
    fn zero_cycle_replays_immediately() {
        let mut timer = Timer::new(Duration::ZERO, TimerMode::Once);
        timer.tick(Duration::from_millis(1));
        assert!(timer.is_finished());
        assert_eq!(
            next_sound_action(SoundCycleInput::Silent {
                timer_finished: timer.is_finished(),
                in_range: true,
            }),
            SoundAction::Play
        );
    }
}
