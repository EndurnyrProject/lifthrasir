use bevy::prelude::*;
use bevy_extended_ui::widgets::{InputField, UIWidgetState};
use game_engine::domain::input::UiFocus;

/// Mirrors extended_ui's focused text-input state into the engine's `UiFocus`
/// so gameplay input is gated while the player is typing in any screen.
///
/// Shared across every screen that hosts text inputs (login, char-create, chat),
/// so it is registered once here rather than per-screen.
pub struct UiFocusMirrorPlugin;

impl Plugin for UiFocusMirrorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, mirror_text_input_focus);
    }
}

fn mirror_text_input_focus(
    mut focus: ResMut<UiFocus>,
    inputs: Query<&UIWidgetState, With<InputField>>,
) {
    focus.text_input_active = inputs.iter().any(|state| state.focused);
}
