pub mod cursor;
pub mod resources;
pub mod systems;

use bevy::prelude::*;
pub use cursor::{CurrentCursorType, CursorChangeRequest, CursorType};
pub use resources::{ForwardedCursorPosition, ForwardedMouseClick};

/// Plugin that handles all input from Tauri UI
/// This includes cursor position, mouse clicks, and terrain cursor visualization
pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ForwardedCursorPosition>()
            .init_resource::<ForwardedMouseClick>()
            .init_resource::<CurrentCursorType>();

        app.add_message::<CursorChangeRequest>();

        app.add_systems(
            Update,
            (
                systems::render_terrain_cursor,
                systems::handle_terrain_click,
                (
                    systems::update_cursor_for_terrain,
                    cursor::handle_cursor_change_requests,
                )
                    .chain(),
            ),
        );

        info!("✅ InputPlugin registered - cursor forwarding, terrain cursor, click handling, and cursor state active");
    }
}
