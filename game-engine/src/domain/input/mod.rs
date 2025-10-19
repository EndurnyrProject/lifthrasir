pub mod resources;
pub mod systems;

use bevy::prelude::*;
pub use resources::{ForwardedCursorPosition, ForwardedMouseClick};

/// Plugin that handles all input from Tauri UI
/// This includes cursor position, mouse clicks, and terrain cursor visualization
pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ForwardedCursorPosition>()
            .init_resource::<ForwardedMouseClick>();
        app.add_systems(
            Update,
            (
                systems::render_terrain_cursor,
                systems::handle_terrain_click,
            ),
        );

        info!("✅ InputPlugin registered - cursor forwarding, terrain cursor, and click handling active");
    }
}
