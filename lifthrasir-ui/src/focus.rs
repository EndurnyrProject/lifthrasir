use bevy::input_focus::InputFocus;
use bevy::input_focus::tab_navigation::TabIndex;
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
        // Feathers installs `TabNavigationPlugin`, whose `click_to_focus` observer
        // fires an `AcquireFocus` on every pointer press. That event bubbles up
        // looking for a `TabIndex`; if it reaches the window it CLEARS `InputFocus`,
        // undoing the click-to-focus that `EditableTextInputPlugin` just performed.
        // Requiring `TabIndex` on every `EditableText` makes the field itself the
        // `AcquireFocus` target, so clicking an input focuses it and clicking
        // anywhere else releases it.
        app.register_required_components::<EditableText, TabIndex>()
            .add_systems(Update, mirror_text_input_focus);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editable_text_is_click_focusable_via_tab_index() {
        let mut app = App::new();
        app.init_resource::<InputFocus>();
        app.init_resource::<UiFocus>();
        app.add_plugins(UiFocusMirrorPlugin);
        let field = app.world_mut().spawn(EditableText::new("")).id();

        assert_eq!(app.world().get::<TabIndex>(field), Some(&TabIndex(0)));
    }
}
