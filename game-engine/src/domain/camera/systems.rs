use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;

use super::components::{CameraFollowSettings, CameraFollowTarget};
use super::resources::CameraRotationDelta;
use crate::domain::entities::character::kinds::CharacterRoot;
use crate::domain::entities::markers::LocalPlayer;
use crate::domain::entities::sprite_rendering::SpriteObjectTree;

/// Type alias for player query with added SpriteObjectTree
type PlayerReadyQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static SpriteObjectTree),
    (With<LocalPlayer>, Added<SpriteObjectTree>),
>;

/// Resource to track if the camera has been spawned.
/// Prevents multiple camera entities from being created.
///
/// # Purpose
/// - Ensures only one camera is spawned per game session
/// - Prevents duplicate cameras if the system runs multiple times
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct CameraSpawned(pub bool);

/// System that spawns the camera entity when the local player character sprite hierarchy is ready.
///
/// # Behavior
/// - Runs once when the local player character has a SpriteObjectTree component
/// - Creates a camera positioned relative to the player's root transform
/// - Sets up follow target and settings components
/// - Prevents duplicate camera spawns via CameraSpawned resource
///
/// # Run Conditions
/// - Player entity with LocalPlayer + SpriteObjectTree exists
/// - Camera not already spawned (CameraSpawned resource = false)
/// - Runs in PostUpdate schedule
pub fn spawn_camera_on_player_ready(
    mut commands: Commands,
    player_query: PlayerReadyQuery,
    root_query: Query<&Transform, With<CharacterRoot>>,
    camera_query: Query<Entity, With<Camera3d>>,
    mut camera_spawned: ResMut<CameraSpawned>,
) {
    // Check if camera already exists
    if !camera_query.is_empty() {
        if !camera_spawned.0 {
            info!("Camera already exists, marking as spawned");
            camera_spawned.0 = true;
        }
        return;
    }

    // Check if camera spawn was already handled
    if camera_spawned.0 {
        return;
    }

    // Find player entity with CharacterObjectTree
    let Ok((_player_entity, object_tree)) = player_query.single() else {
        return;
    };

    // Get the Transform from the character's root entity
    let Ok(root_transform) = root_query.get(object_tree.root) else {
        warn!("Player character root entity missing Transform component");
        return;
    };

    let player_position = root_transform.translation;

    // Create camera settings with default RO-style offset
    let mut settings = CameraFollowSettings::default();

    // Initialize yaw and pitch from the default offset
    // This ensures rotation state matches the initial offset direction
    let offset = settings.offset;
    let distance = offset.length();
    if distance > 0.001 {
        // Calculate yaw from X and Z components
        settings.yaw = offset.x.atan2(-offset.z);
        // Calculate pitch from Y component (negative because NEG_Y is up)
        settings.pitch = (-offset.y / distance).asin();
    }

    // Calculate initial camera position
    // Offset is relative to player, so we add it to player position
    let camera_position = player_position + settings.offset;

    info!(
        "Spawning character-follow camera at {:?}, following player at {:?}",
        camera_position, player_position
    );

    // Spawn camera entity with all required components
    // Note: CameraFollowTarget stores the root entity (which has Transform), not player entity
    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(camera_position).looking_at(player_position, Vec3::NEG_Y),
        CameraFollowTarget::new(object_tree.root, player_position),
        settings,
        Name::new("FollowCamera"),
    ));

    camera_spawned.0 = true;

    info!("Character-follow camera spawned successfully");
}

/// System that updates the cached position of the follow target each frame.
///
/// # Purpose
/// - Caches the player's current position in the CameraFollowTarget component
/// - Prevents the follow system from needing to query the player's Transform
/// - Runs before the camera follow system for consistent positioning
///
/// # Behavior
/// - Queries all cameras with CameraFollowTarget
/// - Updates cached_position from the target entity's Transform (CharacterRoot)
/// - Handles missing target entities gracefully (warns once)
///
/// # Ordering
/// Must run BEFORE `camera_follow_system` via `.chain()` or explicit ordering
pub fn update_camera_target_cache(
    mut camera_query: Query<&mut CameraFollowTarget>,
    target_query: Query<&Transform, With<CharacterRoot>>,
) {
    for mut follow_target in camera_query.iter_mut() {
        if let Ok(target_transform) = target_query.get(follow_target.target_entity) {
            follow_target.cached_position = target_transform.translation;
        } else {
            // Only warn if target entity doesn't exist (shouldn't happen normally)
            // This is a critical error that needs attention
            warn!(
                "Camera follow target entity {:?} not found or missing Transform",
                follow_target.target_entity
            );
        }
    }
}

/// Main camera follow system with smooth interpolation, zoom control, and rotation.
///
/// # Features
/// - Smooth exponential decay interpolation for camera movement
/// - Mouse wheel zoom in/out (adjusts offset magnitude)
/// - Right-click drag rotation (spherical coordinates)
/// - R key to reset zoom AND rotation to default
/// - Prevents NaN values and gimbal lock
///
/// # Behavior
/// 1. **Rotation**: Right-click drag updates yaw/pitch
///    - Converts mouse deltas to angle changes (sensitivity-based)
///    - Clamps pitch to prevent camera flipping
///    - Converts spherical to Cartesian coordinates for offset
///
/// 2. **Zoom**: Mouse wheel adjusts offset magnitude
///    - Preserves rotation while changing distance
///    - Clamped to min/max distance limits
///
/// 3. **Follow**: Smoothly moves camera to maintain offset from player
///    - Uses exponential decay: `decay = 1.0 - exp(-speed * dt)`
///    - Lerps between current and target position
///
/// 4. **Reset**: R key resets zoom and rotation to default
///
/// # Ordering
/// Must run AFTER `update_camera_target_cache` to use fresh player position
pub fn camera_follow_system(
    time: Res<Time>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
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
        // Get target position from cache (updated by update_camera_target_cache)
        let target_position = follow_target.cached_position;

        // ========================================
        // CAMERA ROTATION (Right-Click Drag)
        // ========================================
        if rotation_delta.has_delta() {
            // Convert pixel deltas to angle changes (degrees to radians)
            let yaw_change = (rotation_delta.delta_x * settings.rotation_sensitivity).to_radians();
            let pitch_change =
                (rotation_delta.delta_y * settings.rotation_sensitivity).to_radians();

            // Update yaw and pitch
            settings.yaw += yaw_change;
            settings.pitch =
                (settings.pitch + pitch_change).clamp(settings.min_pitch, settings.max_pitch);

            // Get current distance from offset magnitude
            let distance = settings.offset.length();

            // Convert spherical coordinates to Cartesian offset
            // RO coordinate system: Vec3::NEG_Y is up
            // Yaw: rotation around Y axis (horizontal)
            // Pitch: angle from horizontal plane (negative = looking down)
            let offset_x = distance * settings.pitch.cos() * settings.yaw.sin();
            let offset_y = -distance * settings.pitch.sin(); // Negative for NEG_Y up vector
            let offset_z = -distance * settings.pitch.cos() * settings.yaw.cos();

            settings.offset = Vec3::new(offset_x, offset_y, offset_z);

            debug!(
                "Camera rotation updated: yaw={:.2}°, pitch={:.2}°, offset={:?}",
                settings.yaw.to_degrees(),
                settings.pitch.to_degrees(),
                settings.offset
            );

            // Clear deltas after processing
            rotation_delta.clear();
        }

        // ========================================
        // ZOOM CONTROL (Mouse Wheel)
        // ========================================
        let mut zoom_delta = 0.0f32;
        for mouse_wheel in mouse_wheel_events.read() {
            zoom_delta += mouse_wheel.y;
        }

        if zoom_delta.abs() > 0.001 {
            // Get current distance
            let current_distance = settings.offset.length();

            // Prevent NaN when offset is zero
            if current_distance < 0.001 {
                warn!("Camera offset is too small, resetting to default");
                let defaults = CameraFollowSettings::default();
                settings.offset = defaults.offset;
                settings.yaw = defaults.yaw;
                settings.pitch = defaults.pitch;
                continue;
            }

            // Calculate new distance after zoom
            let zoom_amount = zoom_delta * settings.zoom_speed;
            let new_distance = current_distance - zoom_amount;

            // Clamp to min/max distance
            let clamped_distance = new_distance.clamp(settings.min_distance, settings.max_distance);

            // Rebuild offset from spherical coordinates with new distance
            let offset_x = clamped_distance * settings.pitch.cos() * settings.yaw.sin();
            let offset_y = -clamped_distance * settings.pitch.sin();
            let offset_z = -clamped_distance * settings.pitch.cos() * settings.yaw.cos();

            settings.offset = Vec3::new(offset_x, offset_y, offset_z);

            debug!(
                "Zoom changed: distance {} -> {}",
                current_distance, clamped_distance
            );
        }

        // ========================================
        // RESET ZOOM AND ROTATION (R Key)
        // ========================================
        if keyboard_input.just_pressed(KeyCode::KeyR) {
            let defaults = CameraFollowSettings::default();
            settings.offset = defaults.offset;
            settings.yaw = defaults.yaw;
            settings.pitch = defaults.pitch;
            info!(
                "Camera reset to default: yaw={:.2}°, pitch={:.2}°, distance={:.1}",
                settings.yaw.to_degrees(),
                settings.pitch.to_degrees(),
                settings.offset.length()
            );
        }

        // ========================================
        // SMOOTH FOLLOW (Split-Axis Exponential Decay)
        // ========================================

        // Calculate desired camera position (target + offset)
        let desired_position = target_position + settings.offset;
        let current_position = camera_transform.translation;

        // Calculate separate decay factors for horizontal and vertical axes
        // Horizontal (X, Z): Faster, more responsive
        let decay_horizontal = 1.0 - (-settings.horizontal_smoothing_speed * delta).exp();
        let decay_horizontal = decay_horizontal.clamp(0.0, 1.0);

        // Vertical (Y): Slower, prevents harsh height snapping
        let decay_vertical = 1.0 - (-settings.vertical_smoothing_speed * delta).exp();
        let decay_vertical = decay_vertical.clamp(0.0, 1.0);

        // Apply split-axis interpolation
        // Horizontal axes (X, Z) use faster smoothing
        let new_x = current_position
            .x
            .lerp(desired_position.x, decay_horizontal);
        let new_z = current_position
            .z
            .lerp(desired_position.z, decay_horizontal);

        // Vertical axis (Y) uses slower smoothing to prevent snapping
        let new_y = current_position.y.lerp(desired_position.y, decay_vertical);

        let new_position = Vec3::new(new_x, new_y, new_z);

        // Validate for NaN
        if new_position.is_nan() {
            error!(
                "Camera position calculation resulted in NaN! Current: {:?}, Desired: {:?}",
                current_position, desired_position
            );
            continue;
        }

        // Update camera position
        camera_transform.translation = new_position;

        // ========================================
        // SMOOTH LOOK-AT (Prevents Direction Snapping)
        // ========================================

        // Smooth the look-at target to prevent camera rotation snapping
        // when character changes direction or moves to different heights
        // Use moderate speed (6.0) - faster than position smoothing but still smooth
        let look_at_smoothing_speed = 6.0;
        let decay_look_at = 1.0 - (-look_at_smoothing_speed * delta).exp();
        let decay_look_at = decay_look_at.clamp(0.0, 1.0);

        // Smoothly interpolate the look-at point
        let smoothed_look_at = follow_target
            .smoothed_look_at
            .lerp(target_position, decay_look_at);

        // Validate smoothed look-at
        if smoothed_look_at.is_nan() {
            error!(
                "Smoothed look-at calculation resulted in NaN! Current: {:?}, Target: {:?}",
                follow_target.smoothed_look_at, target_position
            );
            continue;
        }

        // Update smoothed look-at in the component for next frame
        follow_target.smoothed_look_at = smoothed_look_at;

        // Look at the smoothed target position (prevents direction snapping)
        // Use Vec3::NEG_Y as up vector for RO camera orientation
        camera_transform.look_at(smoothed_look_at, Vec3::NEG_Y);
    }
}
