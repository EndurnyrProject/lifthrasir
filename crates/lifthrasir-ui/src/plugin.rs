use crate::widgets::placeholder::PlaceholderPlugin;
use bevy::picking::cursor::OverrideCursor;
use bevy::{
    input_focus::tab_navigation::TabIndex,
    input_focus::{InputDispatchPlugin, InputFocusPlugin, pointer_focus::PointerFocusPlugin},
    prelude::*,
    text::EditableText,
};
use bevy_feathers::FeathersPlugins;

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
        // 0.20 split click-to-focus out of InputFocusPlugin; without it a click can
        // clear focus at the window but never acquire it on a TabIndex target.
        if !app.is_plugin_added::<PointerFocusPlugin>() {
            app.add_plugins(PointerFocusPlugin);
        }
        app.init_resource::<OverrideCursor>()
            .add_plugins((FeathersPlugins, PlaceholderPlugin));
    }
}
