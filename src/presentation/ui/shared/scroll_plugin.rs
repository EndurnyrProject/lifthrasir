use super::scroll_systems::*;
use bevy::prelude::*;

/// Plugin for scrollable panel functionality
/// Provides mouse wheel scrolling, scrollbar interaction, and content management
pub struct ScrollPlugin;

impl Plugin for ScrollPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                // Core scroll systems
                calculate_content_height,
                handle_scroll_wheel,
                update_scroll_content_position,
                update_scrollbar_visibility,
                update_scrollbar_thumb,
                handle_scrollbar_drag,
            )
                .chain(), // Run in order to ensure state is consistent
        );
    }
}
