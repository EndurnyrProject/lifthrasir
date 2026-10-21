//! Editor shell: camera, egui layout and input routing.

pub mod camera;
pub mod ui;

use bevy::prelude::*;
use bevy_egui::EguiPrimaryContextPass;
use ui::{EditorUiArea, EditorUiSet, viewport_has_keyboard, viewport_has_pointer};

/// Camera, UI set ordering and the status bar. UI systems run only in
/// `EguiPrimaryContextPass`, so headless apps can add this without egui.
pub struct EditorShellPlugin;

impl Plugin for EditorShellPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EditorUiArea>()
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
                    (camera::toggle_camera_mode, camera::fly_move).run_if(viewport_has_keyboard),
                    (camera::fly_look_input, camera::orbit_input).run_if(viewport_has_pointer),
                ),
            )
            .add_systems(
                EguiPrimaryContextPass,
                ui::status_bar.in_set(EditorUiSet::StatusBar),
            );
    }
}
