//! Bevy input, viewport and UI adapters. Editing rules live in MapDocument;
//! feature plugins own their entities, resources and system registration.

mod collision;
mod props;
mod ui;
mod viewport;

use bevy::prelude::*;

pub(crate) use viewport::MapAssetPath;

pub(crate) struct EditorPresentationPlugin;

impl Plugin for EditorPresentationPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            bevy::picking::mesh_picking::MeshPickingPlugin,
            viewport::ViewportPlugin,
            collision::CollisionEditorPlugin,
            props::PropEditorPlugin,
            ui::EditorUiPlugin,
        ));
    }
}
