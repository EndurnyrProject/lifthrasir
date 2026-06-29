use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::text::EditableText;
use game_engine::domain::input::UiFocus;

use crate::screens::login::TextField;

/// Mirrors text-input focus into the engine's `UiFocus` so gameplay input is gated
/// while the player is typing in any screen.
///
/// Two sources: `EditableText` fields track focus through Bevy's `InputFocus`
/// resource (char-create name, in-game chat), and the login screen's hand-rolled
/// `TextField`s track it themselves. Registered once here since text inputs live on
/// several screens.
pub struct UiFocusMirrorPlugin;

impl Plugin for UiFocusMirrorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, mirror_text_input_focus);
    }
}

fn mirror_text_input_focus(
    mut focus: ResMut<UiFocus>,
    input_focus: Res<InputFocus>,
    crate_inputs: Query<(), With<EditableText>>,
    login_fields: Query<&TextField>,
) {
    let crate_active = input_focus.get().is_some_and(|e| crate_inputs.contains(e));
    let login_active = login_fields.iter().any(|field| field.focused);
    focus.text_input_active = crate_active || login_active;
}
