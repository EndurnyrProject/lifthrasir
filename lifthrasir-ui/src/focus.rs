use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::text::EditableText;
use lifthrasir_client::domain::input::UiFocus;

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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input_focus::tab_navigation::TabIndex;

    fn focus_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        crate::install_test_ui_primitives(&mut app);
        app.init_resource::<UiFocus>()
            .add_plugins(UiFocusMirrorPlugin);
        app
    }

    #[test]
    fn editable_text_is_click_focusable_via_tab_index() {
        let mut app = focus_app();
        let field = app.world_mut().spawn(EditableText::new("")).id();

        assert_eq!(app.world().get::<TabIndex>(field), Some(&TabIndex(0)));
    }

    #[test]
    fn shared_text_focus_still_suppresses_client_gameplay_input() {
        let mut app = focus_app();
        let field = app.world_mut().spawn(EditableText::new("")).id();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(field, bevy::input_focus::FocusCause::Navigated);
        app.update();
        assert!(app.world().resource::<UiFocus>().text_input_active);
        app.world_mut().resource_mut::<InputFocus>().clear();
        app.update();
        assert!(!app.world().resource::<UiFocus>().text_input_active);
    }
}
