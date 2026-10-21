//! Map documents, open/save and the map tools.

pub mod document;
pub mod layout;

use bevy::prelude::*;

/// Map editing. Systems arrive with the open/save and tool modules.
pub struct MapEditorPlugin;

impl Plugin for MapEditorPlugin {
    fn build(&self, _app: &mut App) {}
}
