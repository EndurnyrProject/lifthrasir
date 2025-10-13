/// Window constants
pub const WINDOW_WIDTH: f32 = 1440.0;
pub const WINDOW_HEIGHT: f32 = 1080.0;

/// Sprite rendering constants
pub const SPRITE_SCALE_SMALL: f32 = 3.0;
pub const SPRITE_SIZE_THRESHOLD: u32 = 50;

/// Scale factor for converting RO sprite pixel coordinates to Bevy 3D world units
/// This factor is applied to both sprite dimensions and position offsets to maintain
/// correct spatial relationships in the 3D world
pub const SPRITE_WORLD_SCALE: f32 = 0.2;

/// Animation timing constants
pub const DEFAULT_ANIMATION_DELAY: f32 = 150.0;
pub const MAX_DISPLAYED_ACTIONS: usize = 8;

/// Map and terrain constants
pub const CELL_SIZE: f32 = 10.0;
