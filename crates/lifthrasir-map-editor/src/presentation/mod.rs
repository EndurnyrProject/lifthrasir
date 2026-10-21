//! Bevy input, viewport and UI adapters. Editing rules live in MapDocument;
//! feature plugins own their entities, resources and system registration.

mod collision;
mod props;
mod ui;
mod viewport;

use bevy::prelude::*;
use bevy_feathers::FeathersPlugins;

pub(crate) use viewport::MapAssetPath;

pub(crate) struct EditorPresentationPlugin;

impl Plugin for EditorPresentationPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ui::editor_theme()).add_plugins((
            FeathersPlugins,
            bevy::picking::mesh_picking::MeshPickingPlugin,
            viewport::ViewportPlugin,
            collision::CollisionEditorPlugin,
            props::PropEditorPlugin,
            ui::EditorUiPlugin,
        ));
    }
}
