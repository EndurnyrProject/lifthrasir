use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_init_resource;

/// Tracks whether a UI text input currently holds focus.
///
/// While `text_input_active` is true, gameplay input systems are suppressed so
/// keystrokes and clicks routed to a focused text field do not also drive the game.
#[derive(Resource, Default)]
#[auto_init_resource(plugin = crate::app::input_plugin::InputPlugin)]
pub struct UiFocus {
    pub text_input_active: bool,
}

/// Run condition that is true when no UI text input holds focus.
pub fn ui_unfocused(focus: Res<UiFocus>) -> bool {
    !focus.text_input_active
}
