use bevy::camera::Exposure;
use bevy::input::mouse::MouseWheel;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::Hdr;
use bevy_auto_plugin::prelude::*;

use super::components::{CameraFollowSettings, CameraFollowTarget};
use super::resources::CameraRotationDelta;
use crate::domain::entities::markers::LocalPlayer;
use crate::domain::input::UiFocus;
use crate::domain::system_sets::CameraSystems;

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
    camera_query: Query<Entity, With<Camera3d>>,
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
        // HDR + manual exposure decouple final image brightness from absolute light
        // values; bloom makes bright lights read as glowing. See lighting.rs for the
        // matching sun/ambient/point-light scale.
        Hdr,
        Exposure { ev100: 8.0 },
        Bloom::NATURAL,
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

        // Camera rotation (right-click drag)
        if rotation_delta.has_delta() {
            let yaw_change = (rotation_delta.delta_x * settings.rotation_sensitivity).to_radians();
            let pitch_change =
                (rotation_delta.delta_y * settings.rotation_sensitivity).to_radians();

            settings.yaw += yaw_change;
            settings.pitch =
                (settings.pitch + pitch_change).clamp(settings.min_pitch, settings.max_pitch);

            let distance = settings.offset.length();

            let offset_x = distance * settings.pitch.cos() * settings.yaw.sin();
            let offset_y = -distance * settings.pitch.sin();
            let offset_z = -distance * settings.pitch.cos() * settings.yaw.cos();

            settings.offset = Vec3::new(offset_x, offset_y, offset_z);

            debug!(
                "Camera rotation updated: yaw={:.2}deg, pitch={:.2}deg, offset={:?}",
                settings.yaw.to_degrees(),
                settings.pitch.to_degrees(),
                settings.offset
            );

            rotation_delta.clear();
        }

        // Zoom control (mouse wheel)
        let mut zoom_delta = 0.0f32;
        for mouse_wheel in mouse_wheel_events.read() {
            zoom_delta += mouse_wheel.y;
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

            let zoom_amount = zoom_delta * settings.zoom_speed;
            let new_distance = current_distance - zoom_amount;

            let clamped_distance = new_distance.clamp(settings.min_distance, settings.max_distance);

            let offset_x = clamped_distance * settings.pitch.cos() * settings.yaw.sin();
            let offset_y = -clamped_distance * settings.pitch.sin();
            let offset_z = -clamped_distance * settings.pitch.cos() * settings.yaw.cos();

            settings.offset = Vec3::new(offset_x, offset_y, offset_z);

            debug!(
                "Zoom changed: distance {} -> {}",
                current_distance, clamped_distance
            );
        }

        // Reset zoom and rotation (R key)
        if !ui_focus.text_input_active && keyboard_input.just_pressed(KeyCode::KeyR) {
            let defaults = CameraFollowSettings::default();
            settings.offset = defaults.offset;
            settings.yaw = defaults.yaw;
            settings.pitch = defaults.pitch;
            debug!(
                "Camera reset to default: yaw={:.2}deg, pitch={:.2}deg, distance={:.1}",
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
