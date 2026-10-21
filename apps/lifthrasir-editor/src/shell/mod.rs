//! Editor shell: camera, egui layout and input routing.

pub mod browser;
pub mod camera;
pub mod node;
pub mod preview;
pub mod ui;
pub mod undo;

use bevy::prelude::*;
use bevy_egui::EguiPrimaryContextPass;
use ui::{EditorUiArea, EditorUiSet, viewport_has_keyboard, viewport_has_pointer};

/// Camera, undo history, node ids, UI set ordering and the status bar. UI systems run only in
/// `EguiPrimaryContextPass`, so headless apps can add this without egui.
pub struct EditorShellPlugin;

impl Plugin for EditorShellPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(browser::BrowserPlugin)
            .init_resource::<EditorUiArea>()
            .init_resource::<undo::UndoStack>()
            .init_resource::<node::EditorNodeIds>()
            .configure_sets(
                EguiPrimaryContextPass,
                (
                    EditorUiSet::MenuBar,
                    EditorUiSet::ToolBar,
                    EditorUiSet::LeftPanel,
                    EditorUiSet::RightPanel,
                    EditorUiSet::Windows,
                    EditorUiSet::StatusBar,
                )
                    .chain(),
            )
            .add_systems(Startup, camera::spawn_editor_camera)
            .add_systems(
                Update,
                (
                    (
                        camera::toggle_camera_mode,
                        camera::fly_move,
                        undo::history_keys,
                    )
                        .run_if(viewport_has_keyboard),
                    (camera::fly_look_input, camera::orbit_input).run_if(viewport_has_pointer),
                ),
            )
            .add_systems(
                EguiPrimaryContextPass,
                ui::status_bar.in_set(EditorUiSet::StatusBar),
            );
    }
}
