use bevy::anti_alias::fxaa::Fxaa;
use bevy::prelude::*;
use bevy::window::{
    Monitor, MonitorSelection, PresentMode, PrimaryWindow, VideoMode, VideoModeSelection,
    WindowMode, WindowResolution,
};
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_framepace::FramepaceSettings;
use bevy_persistent::prelude::Persistent;

use super::events::ApplySettings;
use super::resources::{DisplayMode, Settings};
use crate::domain::camera::components::CameraFollowTarget;

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
pub fn apply_graphics(
    mut messages: MessageReader<ApplySettings>,
    settings: Res<Persistent<Settings>>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    monitors: Query<&Monitor>,
    mut framepace: ResMut<FramepaceSettings>,
    cameras: Query<Entity, With<CameraFollowTarget>>,
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
    use super::*;

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
