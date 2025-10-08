pub mod resources;
pub mod systems;

use bevy::prelude::*;
pub use resources::ForwardedCursorPosition;

/// Plugin that handles all input from Tauri UI
/// This includes cursor position, mouse clicks, and terrain cursor visualization
pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ForwardedCursorPosition>();
        app.add_systems(Update, systems::render_terrain_cursor);

        info!("✅ InputPlugin registered - cursor forwarding and terrain cursor active");
    }
}
