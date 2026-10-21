//! Map documents, open/save and the map tools.

pub mod document;
pub mod layout;
pub mod lighting;
pub mod open;
pub mod save;
pub mod wizard;

use bevy::prelude::*;

/// Map editing: the start screen, open, save, save-as and the new map wizard.
pub struct MapEditorPlugin;

impl Plugin for MapEditorPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((open::plugin, save::plugin, wizard::plugin));
    }
}
