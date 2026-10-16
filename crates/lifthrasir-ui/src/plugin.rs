use crate::widgets::placeholder::PlaceholderPlugin;
use bevy::{
    input_focus::tab_navigation::TabIndex,
    input_focus::{InputDispatchPlugin, InputFocusPlugin},
    prelude::*,
    text::EditableText,
};
use bevy_feathers::{FeathersPlugins, cursor::OverrideCursor};

/// Shared widget/focus setup after Bevy asset/input infrastructure and before fields spawn.
/// A host-selected UiTheme must be inserted before this plugin; it is never overwritten here.
pub struct UiPrimitivesPlugin;

impl Plugin for UiPrimitivesPlugin {
    fn build(&self, app: &mut App) {
        // Tab navigation must acquire focus on the field, not clear it at the window.
        app.register_required_components::<EditableText, TabIndex>();
        if !app.is_plugin_added::<InputFocusPlugin>() {
            app.add_plugins(InputFocusPlugin);
        }
        if !app.is_plugin_added::<InputDispatchPlugin>() {
            app.add_plugins(InputDispatchPlugin);
        }
        app.init_resource::<OverrideCursor>()
            .add_plugins((FeathersPlugins, PlaceholderPlugin));
    }
}
