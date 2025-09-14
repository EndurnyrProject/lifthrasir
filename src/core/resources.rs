use bevy::prelude::*;

// Re-export authentication resources for convenience
pub use crate::domain::authentication::models::{AuthenticationContext, ServerConfiguration};
pub use crate::infrastructure::networking::session::UserSession;

#[derive(Resource)]
pub struct GameSettings {
    pub render_distance: f32,
    pub terrain_quality: u8,
    pub lighting_enabled: bool,
    pub water_effects_enabled: bool,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            render_distance: 1000.0,
            terrain_quality: 2,
            lighting_enabled: true,
            water_effects_enabled: true,
        }
    }
}
