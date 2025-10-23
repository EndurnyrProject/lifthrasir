use bevy::prelude::*;

/// Component that links a camera to a follow target (the player character).
/// Caches the target's position to avoid querying it multiple times per frame.
///
/// # Purpose
/// - Establishes a relationship between the camera and the player entity
/// - Provides a cached position for smooth interpolation
/// - Prevents unnecessary entity queries in follow systems
///
/// # Fields
/// - `target_entity`: The entity being followed (typically the player)
/// - `cached_position`: Last known position of the target, updated each frame
/// - `smoothed_look_at`: Smoothed look-at point to prevent camera direction snapping
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct CameraFollowTarget {
    /// The entity this camera is following
    pub target_entity: Entity,
    /// Cached position of the target from the previous frame
    /// Updated by `update_camera_target_cache` system
    pub cached_position: Vec3,
    /// Smoothed look-at point to prevent sudden camera rotation changes
    /// when character changes direction or moves to different terrain heights
    pub smoothed_look_at: Vec3,
}

impl CameraFollowTarget {
    /// Creates a new follow target for the specified entity
    ///
    /// # Arguments
    /// - `target_entity`: The entity to follow (player character)
    /// - `initial_position`: Starting position of the target
    pub fn new(target_entity: Entity, initial_position: Vec3) -> Self {
        Self {
            target_entity,
            cached_position: initial_position,
            smoothed_look_at: initial_position,
        }
    }
}

/// Configuration settings for the camera follow behavior.
///
/// # Purpose
/// Controls how the camera follows the player character with smooth interpolation,
/// zoom limits, rotation, and offset positioning for the Ragnarok Online isometric style.
///
/// # Fields
/// - `offset`: Camera position relative to the player (default: RO isometric style)
/// - `horizontal_smoothing_speed`: Speed for X and Z axis movement (faster)
/// - `vertical_smoothing_speed`: Speed for Y axis movement (slower, prevents height snapping)
/// - `min_distance`: Minimum zoom distance (prevents camera from going too close)
/// - `max_distance`: Maximum zoom distance (prevents camera from going too far)
/// - `zoom_speed`: Speed of zoom changes via mouse wheel
/// - `rotation_sensitivity`: Degrees per pixel for rotation (0.3 recommended)
/// - `yaw`: Current horizontal rotation in radians (0.0 = facing north)
/// - `pitch`: Current vertical rotation in radians (-45° default, looking down)
/// - `min_pitch`: Minimum pitch angle to prevent camera flipping
/// - `max_pitch`: Maximum pitch angle to prevent camera flipping
///
/// # Smoothing Algorithm
/// Uses split-axis exponential decay interpolation:
/// ```ignore
/// let decay_h = 1.0 - (-horizontal_smoothing_speed * delta).exp();
/// let decay_v = 1.0 - (-vertical_smoothing_speed * delta).exp();
/// new_position.xz = current.xz.lerp(target.xz, decay_h);
/// new_position.y = current.y.lerp(target.y, decay_v);
/// ```
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct CameraFollowSettings {
    /// Camera offset from the player character in world space
    /// Default: Vec3::new(0.0, -150.0, -150.0) for RO isometric style
    pub offset: Vec3,

    /// Smoothing speed for horizontal movement (X and Z axes)
    /// Higher values = faster following, lower values = smoother but slower
    /// Recommended: 3.0 - 5.0 for cinematic feel
    pub horizontal_smoothing_speed: f32,

    /// Smoothing speed for vertical movement (Y axis)
    /// Slower than horizontal to prevent harsh height snapping on terrain changes
    /// Recommended: 2.0 - 3.5 for smooth height transitions
    pub vertical_smoothing_speed: f32,

    /// Minimum allowed distance from the player (zoom in limit)
    pub min_distance: f32,

    /// Maximum allowed distance from the player (zoom out limit)
    pub max_distance: f32,

    /// Speed multiplier for zoom operations (mouse wheel)
    pub zoom_speed: f32,

    /// Rotation sensitivity in degrees per pixel (0.3 recommended)
    pub rotation_sensitivity: f32,

    /// Current horizontal rotation (yaw) in radians
    /// 0.0 = camera behind player facing north
    pub yaw: f32,

    /// Current vertical rotation (pitch) in radians
    /// Negative = looking down (RO style: -45° default)
    pub pitch: f32,

    /// Minimum pitch angle in radians (prevents camera flipping)
    pub min_pitch: f32,

    /// Maximum pitch angle in radians (prevents camera flipping)
    pub max_pitch: f32,
}

impl Default for CameraFollowSettings {
    fn default() -> Self {
        use std::f32::consts::PI;

        Self {
            // RO-style isometric camera offset
            // Y=-150 (above player), Z=-150 (behind player)
            offset: Vec3::new(0.0, -150.0, -150.0),

            // Cinematic horizontal smoothing (X, Z axes)
            horizontal_smoothing_speed: 4.0,

            // Slower vertical smoothing (Y axis) to prevent height snapping
            vertical_smoothing_speed: 2.5,

            // Reasonable zoom limits for RO-style gameplay
            min_distance: 100.0,
            max_distance: 500.0,

            // Mouse wheel zoom speed
            zoom_speed: 50.0,

            // Rotation sensitivity: 0.3 degrees per pixel
            rotation_sensitivity: 0.3,

            // Initial rotation: 0 yaw (behind player), -45 degrees pitch (looking down)
            yaw: 0.0,
            pitch: -PI / 4.0, // -45 degrees in radians

            // Pitch limits to prevent gimbal lock (±89 degrees)
            min_pitch: -89.0 * PI / 180.0,
            max_pitch: 89.0 * PI / 180.0,
        }
    }
}

impl CameraFollowSettings {
    /// Creates settings with a custom offset while keeping other defaults
    ///
    /// # Arguments
    /// - `offset`: Custom camera offset from player
    pub fn with_offset(offset: Vec3) -> Self {
        Self {
            offset,
            ..Default::default()
        }
    }

    /// Creates settings with custom smoothing speed
    ///
    /// # Arguments
    /// - `speed`: Smoothing speed for horizontal axes (higher = faster, 3.0-5.0 recommended)
    ///   Vertical speed is automatically set to 62.5% of horizontal for natural feel
    pub fn with_smoothing(speed: f32) -> Self {
        Self {
            horizontal_smoothing_speed: speed,
            vertical_smoothing_speed: speed * 0.625, // 62.5% of horizontal for natural feel
            ..Default::default()
        }
    }

    /// Creates settings with split-axis smoothing speeds
    ///
    /// # Arguments
    /// - `horizontal_speed`: Smoothing speed for X and Z axes (3.0-5.0 recommended)
    /// - `vertical_speed`: Smoothing speed for Y axis (2.0-3.5 recommended)
    pub fn with_split_smoothing(horizontal_speed: f32, vertical_speed: f32) -> Self {
        Self {
            horizontal_smoothing_speed: horizontal_speed,
            vertical_smoothing_speed: vertical_speed,
            ..Default::default()
        }
    }

    /// Creates settings with custom zoom limits
    ///
    /// # Arguments
    /// - `min`: Minimum zoom distance
    /// - `max`: Maximum zoom distance
    pub fn with_zoom_limits(min: f32, max: f32) -> Self {
        Self {
            min_distance: min,
            max_distance: max,
            ..Default::default()
        }
    }
}
