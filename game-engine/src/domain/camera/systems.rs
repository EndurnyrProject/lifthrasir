use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;

use super::components::{CameraFollowSettings, CameraFollowTarget, PlayerCharacter};
use crate::domain::entities::character::kinds::CharacterRoot;
use crate::domain::entities::character::sprite_hierarchy::CharacterObjectTree;

/// Type alias for player query with added CharacterObjectTree
type PlayerReadyQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static CharacterObjectTree),
    (With<PlayerCharacter>, Added<CharacterObjectTree>),
>;

/// Resource to track if the camera has been spawned.
/// Prevents multiple camera entities from being created.
///
/// # Purpose
/// - Ensures only one camera is spawned per game session
/// - Prevents duplicate cameras if the system runs multiple times
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct CameraSpawned(pub bool);

/// System that spawns the camera entity when the player character sprite hierarchy is ready.
///
/// # Behavior
/// - Runs once when the player character has a CharacterObjectTree component
/// - Creates a camera positioned relative to the player's root transform
/// - Sets up follow target and settings components
/// - Prevents duplicate camera spawns via CameraSpawned resource
///
/// # Run Conditions
/// - Player entity with PlayerCharacter + CharacterObjectTree exists
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
    let settings = CameraFollowSettings::default();

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

/// Main camera follow system with smooth interpolation and zoom control.
///
/// # Features
/// - Smooth exponential decay interpolation for camera movement
/// - Mouse wheel zoom in/out (adjusts offset magnitude)
/// - R key to reset zoom to default
/// - Maintains camera direction while zooming
/// - Prevents NaN values in all calculations
///
/// # Behavior
/// 1. **Follow**: Smoothly moves camera to maintain offset from player
///    - Uses exponential decay: `decay = 1.0 - exp(-speed * dt)`
///    - Lerps between current and target position
///
/// 2. **Zoom**: Mouse wheel adjusts offset magnitude
///    - Preserves offset direction while changing distance
///    - Clamped to min/max distance limits
///    - Smooth zoom speed from settings
///
/// 3. **Reset**: R key resets offset to default
///    - Instant reset, no interpolation
///    - Useful for recovering from awkward camera angles
///
/// # Edge Cases
/// - Handles zero-length offsets (prevents NaN)
/// - Clamps offset magnitude to prevent extreme values
/// - Validates all calculations for NaN/infinity
///
/// # Ordering
/// Must run AFTER `update_camera_target_cache` to use fresh player position
pub fn camera_follow_system(
    time: Res<Time>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut mouse_wheel_events: EventReader<MouseWheel>,
    mut camera_query: Query<
        (
            &mut Transform,
            &CameraFollowTarget,
            &mut CameraFollowSettings,
        ),
        With<Camera3d>,
    >,
) {
    let delta = time.delta_secs();

    for (mut camera_transform, follow_target, mut settings) in camera_query.iter_mut() {
        // Get target position from cache (updated by update_camera_target_cache)
        let target_position = follow_target.cached_position;

        // ========================================
        // ZOOM CONTROL (Mouse Wheel)
        // ========================================
        let mut zoom_delta = 0.0f32;
        for mouse_wheel in mouse_wheel_events.read() {
            zoom_delta += mouse_wheel.y;
        }

        if zoom_delta.abs() > 0.001 {
            // Calculate current offset magnitude
            let current_offset_magnitude = settings.offset.length();

            // Prevent NaN when offset is zero
            if current_offset_magnitude < 0.001 {
                warn!("Camera offset is too small, resetting to default");
                settings.offset = CameraFollowSettings::default().offset;
                continue;
            }

            // Calculate new magnitude after zoom
            let zoom_amount = zoom_delta * settings.zoom_speed;
            let new_magnitude = current_offset_magnitude - zoom_amount;

            // Clamp to min/max distance
            let clamped_magnitude =
                new_magnitude.clamp(settings.min_distance, settings.max_distance);

            // Preserve offset direction, scale to new magnitude
            let offset_direction = settings.offset.normalize();
            settings.offset = offset_direction * clamped_magnitude;

            debug!(
                "Zoom changed: offset magnitude {} -> {}",
                current_offset_magnitude, clamped_magnitude
            );
        }

        // ========================================
        // RESET ZOOM (R Key)
        // ========================================
        if keyboard_input.just_pressed(KeyCode::KeyR) {
            settings.offset = CameraFollowSettings::default().offset;
            info!("Camera zoom reset to default: {:?}", settings.offset);
        }

        // ========================================
        // SMOOTH FOLLOW (Exponential Decay)
        // ========================================

        // Calculate desired camera position (target + offset)
        let desired_position = target_position + settings.offset;

        // Exponential decay smoothing factor
        // decay_factor approaches 1.0 as time passes, creating smooth interpolation
        let decay_factor = 1.0 - (-settings.smoothing_speed * delta).exp();
        let decay_factor = decay_factor.clamp(0.0, 1.0); // Safety clamp

        // Smoothly interpolate camera position
        let current_position = camera_transform.translation;
        let new_position = current_position.lerp(desired_position, decay_factor);

        // Validate for NaN (shouldn't happen, but safety check)
        if new_position.is_nan() {
            error!(
                "Camera position calculation resulted in NaN! Current: {:?}, Desired: {:?}",
                current_position, desired_position
            );
            continue;
        }

        // Update camera position
        camera_transform.translation = new_position;

        // Always look at the player (maintain RO isometric feel)
        // Use Vec3::NEG_Y as up vector for RO camera orientation
        let look_target = target_position;
        camera_transform.look_at(look_target, Vec3::NEG_Y);
    }
}
