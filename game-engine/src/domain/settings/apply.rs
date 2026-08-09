use bevy::anti_alias::fxaa::Fxaa;
use bevy::camera::Hdr;
use bevy::ecs::system::SystemParam;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::ui::IsDefaultUiCamera;
use bevy::window::{
    Monitor, MonitorSelection, PresentMode, PrimaryWindow, VideoMode, VideoModeSelection,
    WindowMode, WindowResolution,
};
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_framepace::FramepaceSettings;

use leafwing_input_manager::prelude::InputMap;

use super::events::ApplySettings;
use super::resources::{AntiAliasing, AudioConfig, DisplayMode, GraphicsSettings, Keybinds, Ssao};
use crate::domain::audio::{
    AudioSettings, MuteAmbienceEvent, MuteBgmEvent, MuteSfxEvent, SetAmbienceVolumeEvent,
    SetBgmVolumeEvent, SetSfxVolumeEvent,
};
use crate::domain::camera::components::CameraFollowTarget;
use crate::domain::entities::markers::LocalPlayer;
use crate::domain::input::PlayerAction;

use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::core_pipeline::prepass::{DepthPrepass, MotionVectorPrepass, NormalPrepass};
use bevy::pbr::ScreenSpaceAmbientOcclusion;
use bevy::render::camera::{MipBias, TemporalJitter};

#[cfg(feature = "dlss")]
use super::resources::DlssMode;
#[cfg(feature = "dlss")]
use bevy::anti_alias::dlss::{Dlss, DlssSuperResolutionSupported};

/// Index of the candidate nearest (squared pixel distance) to `target`, or
/// `None` when there are no candidates. An exact match wins outright.
fn nearest_mode_index(candidates: &[(u32, u32)], target: (u32, u32)) -> Option<usize> {
    candidates
        .iter()
        .enumerate()
        .min_by_key(|&(_, &(w, h))| {
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

#[auto_add_system(plugin = super::SettingsRuntimePlugin, schedule = Update)]
#[allow(clippy::too_many_arguments)]
pub fn apply_graphics(
    mut messages: MessageReader<ApplySettings>,
    settings: Res<GraphicsSettings>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    monitors: Query<&Monitor>,
    mut framepace: ResMut<FramepaceSettings>,
    cameras: Query<Entity, With<CameraFollowTarget>>,
    ui_cameras: Query<Entity, With<IsDefaultUiCamera>>,
    mut lights: Query<&mut DirectionalLight>,
    mut commands: Commands,
    #[cfg(feature = "dlss")] dlss_supported: Option<Res<DlssSuperResolutionSupported>>,
) {
    if messages.read().count() == 0 {
        return;
    }

    let graphics = &*settings;

    #[cfg(feature = "dlss")]
    let dlss_active = dlss_supported.is_some() && graphics.dlss != DlssMode::Off;
    #[cfg(not(feature = "dlss"))]
    let dlss_active = false;
    #[cfg(feature = "dlss")]
    if graphics.dlss != DlssMode::Off && dlss_supported.is_none() {
        info!("DLSS requested but not supported on this system; leaving it off");
    }

    for mut light in &mut lights {
        if light.shadow_maps_enabled != graphics.shadows {
            light.shadow_maps_enabled = graphics.shadows;
        }
    }

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
        apply_camera_effects(&mut commands, camera, graphics, dlss_active);
    }

    // The UI camera shares the window target with the world camera, so their
    // MSAA sample counts and HDR must match or the world pass fails to
    // composite. FXAA stays world-only.
    let ui_msaa = effective_msaa(graphics, dlss_active);
    let ui_hdr = needs_hdr(graphics, dlss_active);
    for ui_camera in &ui_cameras {
        let mut entity = commands.entity(ui_camera);
        entity.insert(ui_msaa);
        if ui_hdr {
            entity.insert(Hdr);
        } else {
            entity.remove::<Hdr>();
        }
    }

    commands.insert_resource(UiScale(graphics.ui_scaling.to_scale_factor()));
}

/// Mirrors `AudioConfig` into the live `AudioSettings` resource
/// and emits the existing volume/mute events so kira updates playback live.
/// `ambient` (config) maps to `ambience` (runtime); `sfx` maps straight across.
/// The six audio control events `apply_audio` emits so kira updates playback
/// live, grouped so the system takes one parameter instead of six.
#[derive(SystemParam)]
pub struct AudioControlWriters<'w> {
    set_bgm: MessageWriter<'w, SetBgmVolumeEvent>,
    set_sfx: MessageWriter<'w, SetSfxVolumeEvent>,
    set_ambience: MessageWriter<'w, SetAmbienceVolumeEvent>,
    mute_bgm: MessageWriter<'w, MuteBgmEvent>,
    mute_sfx: MessageWriter<'w, MuteSfxEvent>,
    mute_ambience: MessageWriter<'w, MuteAmbienceEvent>,
}

#[auto_add_system(plugin = super::SettingsRuntimePlugin, schedule = Update)]
pub fn apply_audio(
    mut messages: MessageReader<ApplySettings>,
    settings: Res<AudioConfig>,
    mut audio: ResMut<AudioSettings>,
    mut writers: AudioControlWriters,
) {
    if messages.read().count() == 0 {
        return;
    }

    let config = *settings;

    audio.bgm_volume = config.bgm_volume;
    audio.bgm_muted = config.bgm_muted;
    audio.sfx_volume = config.sfx_volume;
    audio.sfx_muted = config.sfx_muted;
    audio.ambience_volume = config.ambient_volume;
    audio.ambience_muted = config.ambient_muted;

    writers.set_bgm.write(SetBgmVolumeEvent {
        volume: config.bgm_volume,
    });
    writers.set_sfx.write(SetSfxVolumeEvent {
        volume: config.sfx_volume,
    });
    writers.set_ambience.write(SetAmbienceVolumeEvent {
        volume: config.ambient_volume,
    });
    writers.mute_bgm.write(MuteBgmEvent {
        muted: config.bgm_muted,
    });
    writers.mute_sfx.write(MuteSfxEvent {
        muted: config.sfx_muted,
    });
    writers.mute_ambience.write(MuteAmbienceEvent {
        muted: config.ambient_muted,
    });
}

/// Rebuilds the local player's `InputMap<PlayerAction>` from the persisted
/// keybinds on `ApplySettings`. No-op when the player has not spawned yet (the
/// spawn site already seeds the map from settings).
#[auto_add_system(plugin = super::SettingsRuntimePlugin, schedule = Update)]
pub fn apply_input(
    mut messages: MessageReader<ApplySettings>,
    settings: Res<Keybinds>,
    mut player: Query<&mut InputMap<PlayerAction>, With<LocalPlayer>>,
) {
    if messages.read().count() == 0 {
        return;
    }
    let Ok(mut input_map) = player.single_mut() else {
        return;
    };
    *input_map = settings.to_input_map();
}

/// Applies the current graphics settings to a freshly-spawned world camera, since
/// the startup `ApplySettings` fires before the camera (which only spawns on
/// entering InGame) exists.
#[auto_add_system(plugin = super::SettingsRuntimePlugin, schedule = Update)]
pub fn apply_camera_effects_on_spawn(
    settings: Res<GraphicsSettings>,
    cameras: Query<Entity, Added<CameraFollowTarget>>,
    mut commands: Commands,
    #[cfg(feature = "dlss")] dlss_supported: Option<Res<DlssSuperResolutionSupported>>,
) {
    #[cfg(feature = "dlss")]
    let dlss_active = dlss_supported.is_some() && settings.dlss != DlssMode::Off;
    #[cfg(not(feature = "dlss"))]
    let dlss_active = false;
    for camera in &cameras {
        apply_camera_effects(&mut commands, camera, &settings, dlss_active);
    }
}

/// Applies the shadow setting to a freshly-spawned directional light, since the
/// map's sun is spawned (with shadows on) per map load, after the last
/// `ApplySettings`.
#[auto_add_system(plugin = super::SettingsRuntimePlugin, schedule = Update)]
pub fn apply_shadows_on_spawn(
    settings: Res<GraphicsSettings>,
    mut lights: Query<&mut DirectionalLight, Added<DirectionalLight>>,
) {
    let shadows = settings.shadows;
    for mut light in &mut lights {
        if light.shadow_maps_enabled != shadows {
            light.shadow_maps_enabled = shadows;
        }
    }
}

/// Whether the world camera needs the HDR pipeline: bloom reads it and DLSS
/// requires it. The UI camera must match (it shares the window render target).
fn needs_hdr(graphics: &GraphicsSettings, dlss_active: bool) -> bool {
    graphics.bloom || dlss_active
}

/// The world camera's effective MSAA. DLSS, TAA, and SSAO each rely on the
/// depth/normal prepass, which is incompatible with MSAA, so any of them forces
/// it off. The UI camera must match (it shares the window render target).
fn effective_msaa(graphics: &GraphicsSettings, dlss_active: bool) -> Msaa {
    if dlss_active || graphics.antialiasing == AntiAliasing::Taa || graphics.ssao != Ssao::Off {
        Msaa::Off
    } else {
        graphics.antialiasing.to_msaa_fxaa().0
    }
}

fn apply_camera_effects(
    commands: &mut Commands,
    camera: Entity,
    graphics: &GraphicsSettings,
    dlss_active: bool,
) {
    let wants_taa = !dlss_active && graphics.antialiasing == AntiAliasing::Taa;
    let ssao_level = graphics.ssao.to_quality_level();
    let ssao_on = graphics.ssao != Ssao::Off;
    // DLSS and TAA are both temporal antialiasers sharing the same prepass set.
    let temporal = dlss_active || wants_taa;

    let msaa = effective_msaa(graphics, dlss_active);
    // FXAA is post-process (no prepass), so it coexists with SSAO; DLSS and TAA
    // are themselves the antialiaser and suppress it.
    let has_fxaa = !temporal && graphics.antialiasing == AntiAliasing::Fxaa;
    let mut entity = commands.entity(camera);
    entity.insert(msaa);
    if has_fxaa {
        entity.insert(Fxaa::default());
    } else {
        entity.remove::<Fxaa>();
    }

    #[cfg(feature = "dlss")]
    if dlss_active {
        if let Some(perf_quality_mode) = graphics.dlss.to_perf_quality_mode() {
            entity.insert(Dlss {
                perf_quality_mode,
                ..default()
            });
        }
    } else {
        entity.remove::<Dlss>();
    }

    if wants_taa {
        entity.insert(TemporalAntiAliasing::default());
    } else {
        entity.remove::<TemporalAntiAliasing>();
    }

    if let Some(quality_level) = ssao_level {
        entity.insert(ScreenSpaceAmbientOcclusion {
            quality_level,
            ..default()
        });
    } else {
        entity.remove::<ScreenSpaceAmbientOcclusion>();
    }

    // DLSS, TAA, and SSAO each pull these prepass/temporal components in via their
    // required components; strip them only when no remaining consumer needs them.
    if !temporal && !ssao_on {
        entity.remove::<DepthPrepass>();
    }
    if !temporal {
        entity.remove::<(MotionVectorPrepass, TemporalJitter, MipBias)>();
    }
    if !ssao_on {
        entity.remove::<NormalPrepass>();
    }

    if needs_hdr(graphics, dlss_active) {
        entity.insert(Hdr);
    } else {
        entity.remove::<Hdr>();
    }
    if graphics.bloom {
        entity.insert(Bloom::NATURAL);
    } else {
        entity.remove::<Bloom>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audio_test_app(config: AudioConfig) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<AudioSettings>();
        app.insert_resource(config);
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
        let config = AudioConfig {
            bgm_volume: 0.1,
            bgm_muted: true,
            sfx_volume: 0.2,
            sfx_muted: false,
            ambient_volume: 0.3,
            ambient_muted: true,
        };
        let mut app = audio_test_app(config);
        app.world_mut().write_message(ApplySettings);
        app.update();

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
        let mut app = audio_test_app(AudioConfig::default());
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
    fn hdr_follows_bloom_and_dlss() {
        let mut graphics = GraphicsSettings {
            bloom: false,
            ..default()
        };
        assert!(!needs_hdr(&graphics, false));
        assert!(needs_hdr(&graphics, true));
        graphics.bloom = true;
        assert!(needs_hdr(&graphics, false));
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
