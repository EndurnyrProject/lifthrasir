use bevy::prelude::*;

/// Animation quality settings for performance tuning
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationQuality {
    /// Maximum quality - all optimizations disabled
    /// Updates every frame, no culling
    Ultra,

    /// High quality - visibility culling enabled
    /// Standard frame rate
    High,

    /// Medium quality - visibility + distance culling
    /// Reduced frame rate for distant entities
    Medium,

    /// Low quality - aggressive culling
    /// Reduced frame rate for all non-essential entities
    Low,

    /// Performance mode - minimal animation updates
    /// Only update visible, close entities
    Performance,
}

impl Default for AnimationQuality {
    fn default() -> Self {
        Self::High
    }
}

#[derive(Resource, Debug, Clone)]
pub struct AnimationSettings {
    pub quality: AnimationQuality,

    /// Maximum distance for full-rate animations
    pub max_full_rate_distance: f32,

    /// Maximum distance for reduced-rate animations
    pub max_reduced_rate_distance: f32,

    /// Frame skip factor for distant entities (update every N frames)
    pub distant_frame_skip: u32,
}

impl Default for AnimationSettings {
    fn default() -> Self {
        Self {
            quality: AnimationQuality::High,
            max_full_rate_distance: 1000.0,
            max_reduced_rate_distance: 2000.0,
            distant_frame_skip: 2,
        }
    }
}
