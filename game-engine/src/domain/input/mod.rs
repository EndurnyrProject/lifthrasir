pub mod cursor;
pub mod resources;
pub mod systems;
pub mod terrain_raycast;

use bevy::prelude::*;
use crate::core::state::GameState;
pub use cursor::{CurrentCursorType, CursorChangeRequest, CursorType};
pub use resources::{ForwardedCursorPosition, ForwardedMouseClick};
pub use terrain_raycast::TerrainRaycastCache;

/// Plugin that handles all input from Tauri UI
/// This includes cursor position, mouse clicks, and terrain cursor visualization
pub struct InputPlugin;

/// System to set default cursor for UI states
fn set_default_cursor_for_ui(mut cursor_messages: MessageWriter<CursorChangeRequest>) {
    cursor_messages.write(CursorChangeRequest::new(CursorType::Default));
}

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ForwardedCursorPosition>()
            .init_resource::<ForwardedMouseClick>()
            .init_resource::<CurrentCursorType>()
            .init_resource::<TerrainRaycastCache>();

        app.add_message::<CursorChangeRequest>();

        app.add_systems(
            Update,
            (
                terrain_raycast::update_terrain_raycast_cache,
                systems::render_terrain_cursor,
                systems::handle_terrain_click,
                (
                    systems::update_cursor_for_terrain.run_if(in_state(GameState::InGame)),
                    cursor::handle_cursor_change_requests,
                )
                    .chain(),
            )
                .chain(),
        );

        app.add_systems(OnEnter(GameState::Login), set_default_cursor_for_ui);
        app.add_systems(OnEnter(GameState::ServerSelection), set_default_cursor_for_ui);
        app.add_systems(OnEnter(GameState::CharacterSelection), set_default_cursor_for_ui);

        info!("✅ InputPlugin registered - cursor forwarding, terrain cursor, click handling, and cursor state active");
    }
}
