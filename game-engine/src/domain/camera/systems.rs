use bevy::camera::Exposure;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

use super::components::{CameraFollowSettings, CameraFollowTarget};
use super::resources::{ActiveCameraProfile, CameraRotationDelta, IndoorMapTable};
use crate::domain::entities::markers::LocalPlayer;
use crate::domain::input::UiFocus;
use crate::domain::system_sets::CameraSystems;
use crate::domain::world::spawn_context::MapSpawnContext;
use crate::infrastructure::assets::IndoorMapTableAsset;

/// Distance change per discrete zoom step (mouse notch).
const ZOOM_STEP: f32 = 25.0;
/// Pixel-unit scroll deltas (trackpads/precision wheels) are divided by this to
/// normalize them to roughly one line-notch, so magnitude can't fling the zoom.
const PIXEL_NORMALIZE: f32 = 50.0;

/// Indoor camera preset: closer, tighter zoom range, fixed diagonal, no rotation.
const INDOOR_MIN_DISTANCE: f32 = 90.0;
const INDOOR_MAX_DISTANCE: f32 = 130.0;
const INDOOR_DISTANCE: f32 = 110.0;
/// Fixed indoor yaw: camera sits to the southwest looking northeast (RO diagonal).
/// World axes: +X = East, +Z = North, so -45° gives offset (-x, -z) = southwest.
const INDOOR_YAW: f32 = -std::f32::consts::FRAC_PI_4;

/// Build a camera offset vector from yaw/pitch angles and a distance.
fn offset_from_angles(yaw: f32, pitch: f32, distance: f32) -> Vec3 {
    Vec3::new(
        distance * pitch.cos() * yaw.sin(),
        -distance * pitch.sin(),
        -distance * pitch.cos() * yaw.cos(),
    )
}

/// Normalize a map name for indoor-table lookup: strip the extension, lowercase.
fn normalize_map_name(map_name: &str) -> String {
    map_name
        .trim_end_matches(".gat")
        .trim_end_matches(".rsw")
        .to_lowercase()
}

/// Apply the indoor or outdoor camera preset to the follow settings.
/// Both presets reset yaw/pitch to the default diagonal and recompute the offset.
fn apply_camera_profile(settings: &mut CameraFollowSettings, indoor: bool) {
    let defaults = CameraFollowSettings::default();
    settings.rotation_locked = indoor;
    settings.pitch = defaults.pitch;

    let (yaw, min, max, distance) = if indoor {
        (
            INDOOR_YAW,
            INDOOR_MIN_DISTANCE,
            INDOOR_MAX_DISTANCE,
            INDOOR_DISTANCE,
        )
    } else {
        (
            defaults.yaw,
            defaults.min_distance,
            defaults.max_distance,
            defaults.offset.length(),
        )
    };

    settings.yaw = yaw;
    settings.min_distance = min;
    settings.max_distance = max;
    settings.offset = offset_from_angles(yaw, settings.pitch, distance);
}

// =============================================================================
// PHASE 0.2: UPDATED TO USE FLAT ENTITY STRUCTURE
// =============================================================================
// Removed SpriteObjectTree dependency - now queries LocalPlayer entity directly.
// The player entity now has Transform directly (no child hierarchy).
// =============================================================================

/// Type alias for player query - matches local player with Transform
type PlayerReadyQuery<'w, 's> = Query<'w, 's, (Entity, &'static Transform), With<LocalPlayer>>;

/// Resource to track if the camera has been spawned.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct CameraSpawned(pub bool);

/// System that spawns the camera entity when the local player entity is spawned.
pub fn spawn_camera_on_player_ready(
    mut commands: Commands,
    player_query: PlayerReadyQuery,
    camera_query: Query<
        Entity,
        (
            With<Camera3d>,
            Without<crate::domain::entities::billboard::EquipmentPreviewCamera>,
        ),
    >,
    mut camera_spawned: ResMut<CameraSpawned>,
) {
    if !camera_query.is_empty() {
        if !camera_spawned.0 {
            debug!("Camera already exists, marking as spawned");
            camera_spawned.0 = true;
        }
        return;
    }

    if camera_spawned.0 {
        return;
    }

    let Ok((player_entity, player_transform)) = player_query.single() else {
        return;
    };

    let player_position = player_transform.translation;

    let mut settings = CameraFollowSettings::default();

    let offset = settings.offset;
    let distance = offset.length();
    if distance > 0.001 {
        settings.yaw = offset.x.atan2(-offset.z);
        settings.pitch = (-offset.y / distance).asin();
    }

    let camera_position = player_position + settings.offset;

    debug!(
        "Spawning character-follow camera at {:?}, following player at {:?}",
        camera_position, player_position
    );

    commands.spawn((
        Camera3d::default(),
        // Manual exposure decouples final image brightness from absolute light
        // values. HDR + bloom are inserted by the settings layer
        // (apply_camera_effects) per the graphics settings; see lighting.rs for the
        // matching sun/ambient/point-light scale.
        Exposure { ev100: 8.0 },
        Transform::from_translation(camera_position).looking_at(player_position, Vec3::NEG_Y),
        CameraFollowTarget::new(player_entity, player_position),
        settings,
        Name::new("FollowCamera"),
    ));

    camera_spawned.0 = true;

    debug!("Character-follow camera spawned successfully");
}

/// System that updates the cached position of the follow target each frame.
#[auto_add_system(
    plugin = crate::LifthrasirPlugin,
    schedule = Update,
    config(in_set = CameraSystems::TargetUpdate)
)]
pub fn update_camera_target_cache(
    mut camera_query: Query<&mut CameraFollowTarget>,
    target_query: Query<&Transform, With<LocalPlayer>>,
) {
    for mut follow_target in camera_query.iter_mut() {
        if let Ok(target_transform) = target_query.get(follow_target.target_entity) {
            follow_target.cached_position = target_transform.translation;
        } else {
            warn!(
                "Camera follow target entity {:?} not found or missing Transform",
                follow_target.target_entity
            );
        }
    }
}

/// Main camera follow system with smooth interpolation, zoom control, and rotation.
#[auto_add_system(
    plugin = crate::LifthrasirPlugin,
    schedule = Update,
    config(in_set = CameraSystems::Follow)
)]
pub fn camera_follow_system(
    time: Res<Time>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    ui_focus: Res<UiFocus>,
    mut mouse_wheel_events: MessageReader<MouseWheel>,
    mut rotation_delta: ResMut<CameraRotationDelta>,
    active_profile: Res<ActiveCameraProfile>,
    mut camera_query: Query<
        (
            &mut Transform,
            &mut CameraFollowTarget,
            &mut CameraFollowSettings,
        ),
        With<Camera3d>,
    >,
) {
    let delta = time.delta_secs();

    for (mut camera_transform, mut follow_target, mut settings) in camera_query.iter_mut() {
        let target_position = follow_target.cached_position;

        // Camera rotation (right-click drag) — disabled on indoor maps. The delta is
        // cleared regardless so it can't accumulate while locked and snap on unlock.
        if rotation_delta.has_delta() {
            if !settings.rotation_locked {
                let yaw_change =
                    (rotation_delta.delta_x * settings.rotation_sensitivity).to_radians();
                let pitch_change =
                    (rotation_delta.delta_y * settings.rotation_sensitivity).to_radians();

                settings.yaw += yaw_change;
                settings.pitch =
                    (settings.pitch + pitch_change).clamp(settings.min_pitch, settings.max_pitch);

                let distance = settings.offset.length();
                settings.offset = offset_from_angles(settings.yaw, settings.pitch, distance);

                debug!(
                    "Camera rotation updated: yaw={:.2}deg, pitch={:.2}deg, offset={:?}",
                    settings.yaw.to_degrees(),
                    settings.pitch.to_degrees(),
                    settings.offset
                );
            }

            rotation_delta.clear();
        }

        // Zoom control (mouse wheel) — unit-aware, one bounded step per frame so a
        // big trackpad/precision-wheel delta can't fling the zoom.
        let mut zoom_delta = 0.0f32;
        for mouse_wheel in mouse_wheel_events.read() {
            zoom_delta += match mouse_wheel.unit {
                MouseScrollUnit::Line => mouse_wheel.y,
                MouseScrollUnit::Pixel => mouse_wheel.y / PIXEL_NORMALIZE,
            };
        }

        if zoom_delta.abs() > 0.001 {
            let current_distance = settings.offset.length();

            if current_distance < 0.001 {
                warn!("Camera offset is too small, resetting to default");
                let defaults = CameraFollowSettings::default();
                settings.offset = defaults.offset;
                settings.yaw = defaults.yaw;
                settings.pitch = defaults.pitch;
                continue;
            }

            let new_distance = current_distance - zoom_delta.signum() * ZOOM_STEP;
            let clamped_distance = new_distance.clamp(settings.min_distance, settings.max_distance);

            settings.offset = offset_from_angles(settings.yaw, settings.pitch, clamped_distance);

            debug!(
                "Zoom changed: distance {} -> {}",
                current_distance, clamped_distance
            );
        }

        // Reset zoom and rotation (R key) — respects the active map profile so it
        // can't unlock the camera on an indoor map.
        if !ui_focus.text_input_active && keyboard_input.just_pressed(KeyCode::KeyR) {
            apply_camera_profile(&mut settings, active_profile.indoor);
            debug!(
                "Camera reset to {} profile: yaw={:.2}deg, pitch={:.2}deg, distance={:.1}",
                if active_profile.indoor {
                    "indoor"
                } else {
                    "outdoor"
                },
                settings.yaw.to_degrees(),
                settings.pitch.to_degrees(),
                settings.offset.length()
            );
        }

        // Smooth follow
        let desired_position = target_position + settings.offset;
        let current_position = camera_transform.translation;

        let decay_horizontal = 1.0 - (-settings.horizontal_smoothing_speed * delta).exp();
        let decay_horizontal = decay_horizontal.clamp(0.0, 1.0);

        let decay_vertical = 1.0 - (-settings.vertical_smoothing_speed * delta).exp();
        let decay_vertical = decay_vertical.clamp(0.0, 1.0);

        let new_x = current_position
            .x
            .lerp(desired_position.x, decay_horizontal);
        let new_z = current_position
            .z
            .lerp(desired_position.z, decay_horizontal);

        let new_y = current_position.y.lerp(desired_position.y, decay_vertical);

        let new_position = Vec3::new(new_x, new_y, new_z);

        if new_position.is_nan() {
            error!(
                "Camera position calculation resulted in NaN! Current: {:?}, Desired: {:?}",
                current_position, desired_position
            );
            continue;
        }

        camera_transform.translation = new_position;

        // Smooth look-at
        let look_at_smoothing_speed = 6.0;
        let decay_look_at = 1.0 - (-look_at_smoothing_speed * delta).exp();
        let decay_look_at = decay_look_at.clamp(0.0, 1.0);

        let smoothed_look_at = follow_target
            .smoothed_look_at
            .lerp(target_position, decay_look_at);

        if smoothed_look_at.is_nan() {
            error!(
                "Smoothed look-at calculation resulted in NaN! Current: {:?}, Target: {:?}",
                follow_target.smoothed_look_at, target_position
            );
            continue;
        }

        follow_target.smoothed_look_at = smoothed_look_at;

        camera_transform.look_at(smoothed_look_at, Vec3::NEG_Y);
    }
}

/// Load the indoor map table (`data\indoorrswtable.txt`) once at startup.
pub fn load_indoor_map_table(
    mut indoor_table: ResMut<IndoorMapTable>,
    asset_server: Res<AssetServer>,
) {
    if indoor_table.handle.is_none() {
        debug!("Loading indoor map table from ro://data/indoorrswtable.txt");
        indoor_table.handle = Some(asset_server.load("ro://data/indoorrswtable.txt"));
    }
}

/// Apply the indoor/outdoor camera preset when the map changes.
///
/// Re-applies only when the normalized current map differs from the last-applied
/// one, and only once both the indoor table and the follow camera exist. Indoor
/// maps lock rotation to a closer fixed diagonal; outdoor maps reset to the default
/// free camera.
pub fn apply_camera_map_profile(
    spawn_context: Option<Res<MapSpawnContext>>,
    indoor_table: Res<IndoorMapTable>,
    table_assets: Res<Assets<IndoorMapTableAsset>>,
    mut active_profile: ResMut<ActiveCameraProfile>,
    mut camera_query: Query<&mut CameraFollowSettings, With<Camera3d>>,
) {
    let Some(spawn_context) = spawn_context else {
        return;
    };

    let map_name = normalize_map_name(&spawn_context.map_name);
    if map_name == active_profile.map_name {
        return;
    }

    let Some(handle) = &indoor_table.handle else {
        return;
    };
    let Some(table) = table_assets.get(handle) else {
        return;
    };

    let indoor = table.maps.contains(&map_name);

    let mut applied = false;
    for mut settings in camera_query.iter_mut() {
        apply_camera_profile(&mut settings, indoor);
        applied = true;
    }

    if applied {
        debug!(
            "Camera profile for '{}' applied: {}",
            map_name,
            if indoor { "indoor" } else { "outdoor" }
        );
        active_profile.map_name = map_name;
        active_profile.indoor = indoor;
    }
}
