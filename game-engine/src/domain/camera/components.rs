use bevy::prelude::*;

/// Marker component to identify the player's character entity.
/// This is used to distinguish the player's character from other entities
/// like NPCs, monsters, or other players in multiplayer scenarios.
///
/// # Usage
/// ```ignore
/// commands.spawn((
///     CharacterData { ... },
///     PlayerCharacter,
/// ));
/// ```
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct PlayerCharacter;

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
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct CameraFollowTarget {
    /// The entity this camera is following
    pub target_entity: Entity,
    /// Cached position of the target from the previous frame
    /// Updated by `update_camera_target_cache` system
    pub cached_position: Vec3,
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
        }
    }
}

/// Configuration settings for the camera follow behavior.
///
/// # Purpose
/// Controls how the camera follows the player character with smooth interpolation,
/// zoom limits, and offset positioning for the Ragnarok Online isometric style.
///
/// # Fields
/// - `offset`: Camera position relative to the player (default: RO isometric style)
/// - `smoothing_speed`: Speed of exponential decay interpolation (higher = faster)
/// - `min_distance`: Minimum zoom distance (prevents camera from going too close)
/// - `max_distance`: Maximum zoom distance (prevents camera from going too far)
/// - `zoom_speed`: Speed of zoom changes via mouse wheel
///
/// # Smoothing Algorithm
/// Uses exponential decay interpolation:
/// ```ignore
/// let decay_factor = 1.0 - (-smoothing_speed * delta).exp();
/// smoothed_position = current.lerp(target, decay_factor);
/// ```
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct CameraFollowSettings {
    /// Camera offset from the player character in world space
    /// Default: Vec3::new(0.0, -150.0, -150.0) for RO isometric style
    pub offset: Vec3,

    /// Smoothing speed for camera movement (exponential decay)
    /// Higher values = faster following, lower values = smoother but slower
    /// Recommended range: 5.0 - 15.0
    pub smoothing_speed: f32,

    /// Minimum allowed distance from the player (zoom in limit)
    pub min_distance: f32,

    /// Maximum allowed distance from the player (zoom out limit)
    pub max_distance: f32,

    /// Speed multiplier for zoom operations (mouse wheel)
    pub zoom_speed: f32,
}

impl Default for CameraFollowSettings {
    fn default() -> Self {
        Self {
            // RO-style isometric camera offset
            // Y=-150 (above player), Z=-150 (behind player)
            offset: Vec3::new(0.0, -150.0, -150.0),

            // Smooth but responsive following
            smoothing_speed: 10.0,

            // Reasonable zoom limits for RO-style gameplay
            min_distance: 100.0,
            max_distance: 500.0,

            // Mouse wheel zoom speed
            zoom_speed: 50.0,
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
    /// - `speed`: Smoothing speed (higher = faster, 5.0-15.0 recommended)
    pub fn with_smoothing(speed: f32) -> Self {
        Self {
            smoothing_speed: speed,
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
