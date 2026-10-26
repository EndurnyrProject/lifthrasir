//! Map documents, open/save and the map tools.

pub mod autosave;
pub mod document;
pub mod layout;
pub mod lighting;
pub mod open;
pub mod preferences_window;
pub mod refresh;
pub mod save;
pub mod session;
pub mod settings;
pub mod terrain;
pub mod tools;
pub mod wizard;

use bevy::prelude::*;

/// Map editing: the start screen, open, save, save-as, the new map wizard,
/// map settings and the tools.
pub struct MapEditorPlugin;

impl Plugin for MapEditorPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            autosave::plugin,
            open::plugin,
            preferences_window::plugin,
            save::plugin,
            session::plugin,
            settings::plugin,
            terrain::plugin,
            wizard::plugin,
            tools::ToolsPlugin,
        ));
    }
}
