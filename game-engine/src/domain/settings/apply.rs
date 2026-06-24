use bevy::anti_alias::fxaa::Fxaa;
use bevy::prelude::*;
use bevy::ui::IsDefaultUiCamera;
use bevy::window::{
    Monitor, MonitorSelection, PresentMode, PrimaryWindow, VideoMode, VideoModeSelection,
    WindowMode, WindowResolution,
};
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_framepace::FramepaceSettings;
use bevy_persistent::prelude::Persistent;

use leafwing_input_manager::prelude::InputMap;

use super::events::ApplySettings;
use super::resources::{DisplayMode, Settings};
use crate::domain::audio::{
    AudioSettings, MuteAmbienceEvent, MuteBgmEvent, MuteSfxEvent, SetAmbienceVolumeEvent,
    SetBgmVolumeEvent, SetSfxVolumeEvent,
};
use crate::domain::camera::components::CameraFollowTarget;
use crate::domain::entities::markers::LocalPlayer;
use crate::domain::input::PlayerAction;

/// Index of the candidate nearest (squared pixel distance) to `target`, or
/// `None` when there are no candidates. An exact match wins outright.
fn nearest_mode_index(candidates: &[(u32, u32)], target: (u32, u32)) -> Option<usize> {
    candidates
        .iter()
        .enumerate()
        .min_by_key(|(_, &(w, h))| {
            let dw = w.abs_diff(target.0) as u64;
            let dh = h.abs_diff(target.1) as u64;
            dw * dw + dh * dh
        })
        .map(|(i, _)| i)
}

fn nearest_video_mode(modes: &[VideoMode], target: (u32, u32)) -> Option<VideoMode> {
    let sizes: Vec<(u32, u32)> = modes
        .iter()
        .map(|m| (m.physical_size.x, m.physical_size.y))
        .collect();
    nearest_mode_index(&sizes, target).map(|i| modes[i])
}

#[auto_add_system(plugin = super::SettingsPlugin, schedule = Update)]
#[allow(clippy::too_many_arguments)]
pub fn apply_graphics(
    mut messages: MessageReader<ApplySettings>,
    settings: Res<Persistent<Settings>>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    monitors: Query<&Monitor>,
    mut framepace: ResMut<FramepaceSettings>,
    cameras: Query<Entity, With<CameraFollowTarget>>,
    ui_cameras: Query<Entity, With<IsDefaultUiCamera>>,
    mut commands: Commands,
) {
    if messages.read().count() == 0 {
        return;
    }

    let graphics = settings.graphics;

    window.present_mode = if graphics.vsync {
        PresentMode::AutoVsync
    } else {
        PresentMode::AutoNoVsync
    };

    match graphics.display_mode {
        DisplayMode::Fullscreen => {
            let modes = monitors.iter().flat_map(|m| m.video_modes.iter().copied());
            let modes: Vec<VideoMode> = modes.collect();
            let selection = nearest_video_mode(&modes, graphics.resolution)
                .map(VideoModeSelection::Specific)
                .unwrap_or(VideoModeSelection::Current);
            window.mode = WindowMode::Fullscreen(MonitorSelection::Current, selection);
        }
        mode => {
            window.mode = mode.to_window_mode();
            window.resolution = WindowResolution::new(graphics.resolution.0, graphics.resolution.1);
        }
    }

    framepace.limiter = graphics.fps_cap.to_limiter();

    for camera in &cameras {
        apply_camera_aa(&mut commands, camera, &settings);
    }

    // The UI camera shares the window target with the world camera, so their
    // MSAA sample counts must match or the world pass fails to composite (the
    // same rule HDR follows here). FXAA stays world-only.
    let (ui_msaa, _) = graphics.antialiasing.to_msaa_fxaa();
    for ui_camera in &ui_cameras {
        commands.entity(ui_camera).insert(ui_msaa);
    }

    commands.insert_resource(UiScale(graphics.ui_scaling.to_scale_factor()));
}

/// Mirrors the persisted `Settings.audio` into the live `AudioSettings` resource
/// and emits the existing volume/mute events so kira updates playback live.
/// `ambient` (config) maps to `ambience` (runtime); `sfx` maps straight across.
#[auto_add_system(plugin = super::SettingsPlugin, schedule = Update)]
#[allow(clippy::too_many_arguments)]
pub fn apply_audio(
    mut messages: MessageReader<ApplySettings>,
    settings: Res<Persistent<Settings>>,
    mut audio: ResMut<AudioSettings>,
    mut set_bgm: MessageWriter<SetBgmVolumeEvent>,
    mut set_sfx: MessageWriter<SetSfxVolumeEvent>,
    mut set_ambience: MessageWriter<SetAmbienceVolumeEvent>,
    mut mute_bgm: MessageWriter<MuteBgmEvent>,
    mut mute_sfx: MessageWriter<MuteSfxEvent>,
    mut mute_ambience: MessageWriter<MuteAmbienceEvent>,
) {
    if messages.read().count() == 0 {
        return;
    }

    let config = settings.audio;

    audio.bgm_volume = config.bgm_volume;
    audio.bgm_muted = config.bgm_muted;
    audio.sfx_volume = config.sfx_volume;
    audio.sfx_muted = config.sfx_muted;
    audio.ambience_volume = config.ambient_volume;
    audio.ambience_muted = config.ambient_muted;

    set_bgm.write(SetBgmVolumeEvent {
        volume: config.bgm_volume,
    });
    set_sfx.write(SetSfxVolumeEvent {
        volume: config.sfx_volume,
    });
    set_ambience.write(SetAmbienceVolumeEvent {
        volume: config.ambient_volume,
    });
    mute_bgm.write(MuteBgmEvent {
        muted: config.bgm_muted,
    });
    mute_sfx.write(MuteSfxEvent {
        muted: config.sfx_muted,
    });
    mute_ambience.write(MuteAmbienceEvent {
        muted: config.ambient_muted,
    });
}

/// Rebuilds the local player's `InputMap<PlayerAction>` from the persisted
/// keybinds on `ApplySettings`. No-op when the player has not spawned yet (the
/// spawn site already seeds the map from settings).
#[auto_add_system(plugin = super::SettingsPlugin, schedule = Update)]
pub fn apply_input(
    mut messages: MessageReader<ApplySettings>,
    settings: Res<Persistent<Settings>>,
    mut player: Query<&mut InputMap<PlayerAction>, With<LocalPlayer>>,
) {
    if messages.read().count() == 0 {
        return;
    }
    let Ok(mut input_map) = player.single_mut() else {
        return;
    };
    *input_map = settings.keybinds.to_input_map();
}

/// Applies the current AA settings to a freshly-spawned world camera, since the
/// startup `ApplySettings` fires before the camera (which only spawns on
/// entering InGame) exists.
#[auto_add_system(plugin = super::SettingsPlugin, schedule = Update)]
pub fn apply_camera_aa_on_spawn(
    settings: Res<Persistent<Settings>>,
    cameras: Query<Entity, Added<CameraFollowTarget>>,
    mut commands: Commands,
) {
    for camera in &cameras {
        apply_camera_aa(&mut commands, camera, &settings);
    }
}

fn apply_camera_aa(commands: &mut Commands, camera: Entity, settings: &Settings) {
    let (msaa, has_fxaa) = settings.graphics.antialiasing.to_msaa_fxaa();
    let mut entity = commands.entity(camera);
    entity.insert(msaa);
    if has_fxaa {
        entity.insert(Fxaa::default());
    } else {
        entity.remove::<Fxaa>();
    }
}

#[cfg(test)]
mod tests {
    use super::super::resources::AudioConfig;
    use super::*;
    use bevy_persistent::prelude::StorageFormat;

    fn persistent_settings(slug: &str, settings: Settings) -> Persistent<Settings> {
        let path = std::env::temp_dir().join(format!(
            "lifthrasir-apply-audio-{}-{slug}.ron",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        Persistent::<Settings>::builder()
            .name("settings")
            .format(StorageFormat::Ron)
            .path(path)
            .default(settings)
            .build()
            .expect("build persistent settings")
    }

    fn audio_test_app(slug: &str, settings: Settings) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<AudioSettings>();
        app.insert_resource(persistent_settings(slug, settings));
        app.add_message::<ApplySettings>();
        app.add_message::<SetBgmVolumeEvent>();
        app.add_message::<SetSfxVolumeEvent>();
        app.add_message::<SetAmbienceVolumeEvent>();
        app.add_message::<MuteBgmEvent>();
        app.add_message::<MuteSfxEvent>();
        app.add_message::<MuteAmbienceEvent>();
        app.add_systems(Update, apply_audio);
        app
    }

    #[test]
    fn apply_audio_syncs_settings_into_runtime_resource() {
        let settings = Settings {
            audio: AudioConfig {
                bgm_volume: 0.1,
                bgm_muted: true,
                sfx_volume: 0.2,
                sfx_muted: false,
                ambient_volume: 0.3,
                ambient_muted: true,
            },
            ..Default::default()
        };
        let mut app = audio_test_app("sync", settings);
        app.world_mut().write_message(ApplySettings);
        app.update();

        let config = app.world().resource::<Persistent<Settings>>().audio;
        let audio = app.world().resource::<AudioSettings>();
        assert_eq!(audio.bgm_volume, config.bgm_volume);
        assert_eq!(audio.bgm_muted, config.bgm_muted);
        assert_eq!(audio.sfx_volume, config.sfx_volume);
        assert_eq!(audio.sfx_muted, config.sfx_muted);
        assert_eq!(audio.ambience_volume, config.ambient_volume);
        assert_eq!(audio.ambience_muted, config.ambient_muted);
    }

    #[test]
    fn apply_audio_emits_the_six_audio_messages() {
        let mut app = audio_test_app("messages", Settings::default());
        app.world_mut().write_message(ApplySettings);
        app.update();

        assert_eq!(
            app.world().resource::<Messages<SetBgmVolumeEvent>>().len(),
            1
        );
        assert_eq!(
            app.world().resource::<Messages<SetSfxVolumeEvent>>().len(),
            1
        );
        assert_eq!(
            app.world()
                .resource::<Messages<SetAmbienceVolumeEvent>>()
                .len(),
            1
        );
        assert_eq!(app.world().resource::<Messages<MuteBgmEvent>>().len(), 1);
        assert_eq!(app.world().resource::<Messages<MuteSfxEvent>>().len(), 1);
        assert_eq!(
            app.world().resource::<Messages<MuteAmbienceEvent>>().len(),
            1
        );
    }

    #[test]
    fn nearest_picks_exact_match() {
        let modes = [(1280, 720), (1920, 1080), (2560, 1440)];
        assert_eq!(nearest_mode_index(&modes, (1920, 1080)), Some(1));
    }

    #[test]
    fn nearest_picks_closest_when_no_exact_match() {
        let modes = [(1280, 720), (1920, 1080), (3840, 2160)];
        assert_eq!(nearest_mode_index(&modes, (2560, 1440)), Some(1));
    }

    #[test]
    fn nearest_of_empty_is_none() {
        assert_eq!(nearest_mode_index(&[], (1920, 1080)), None);
    }
}
