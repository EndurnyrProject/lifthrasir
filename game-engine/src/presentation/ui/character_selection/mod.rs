pub mod layout;
pub mod sprites;

use bevy::prelude::*;

/// Marker component for character selection 2D camera
#[derive(Component)]
pub struct CharacterSelectionCamera;

/// System to setup the 2D camera for character sprite rendering
pub fn setup_character_selection_camera(mut commands: Commands) {
    commands.spawn((
        Name::new("CharacterSelection2DCamera"),
        CharacterSelectionCamera,
        Camera2d,
        // Default 2D camera position (no need for far z positioning in 2D)
    ));

    info!("🎥 Created character selection 2D camera at default position");
}
