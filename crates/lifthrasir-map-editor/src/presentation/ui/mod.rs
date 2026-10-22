//! Editor-only Dear ImGui composition. File actions, drawing and input policy
//! live in separate modules; none are shared with the game client's UI.
mod files;
mod input;
mod windows;

#[cfg(test)]
mod tests;

use bevy::prelude::*;
use dear_imgui_bevy::prelude::*;

use crate::application::{EditorNotice, EditorSystems};

pub(super) use input::{pointer_captured, viewport_input_allowed};

#[derive(Resource, Default)]
struct EditorUiState {
    settings_open: bool,
}

pub(super) struct EditorUiPlugin;

impl Plugin for EditorUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ImguiPlugin::default())
            .init_resource::<EditorUiState>()
            .init_resource::<files::MapFilePicker>()
            .configure_sets(
                Update,
                (
                    EditorSystems::Edit,
                    EditorSystems::History,
                    EditorSystems::Save,
                )
                    .run_if(viewport_input_allowed),
            )
            .add_systems(
                Update,
                (
                    files::open_file_shortcut.run_if(viewport_input_allowed),
                    files::poll_map_file_picker,
                    update_notice.in_set(EditorSystems::Present),
                ),
            );
        let pass = app.imgui_primary_pass().expect("editor ImGui primary pass");
        app.add_imgui_systems(
            &pass,
            (
                pass.system(windows::draw),
                pass.system(input::sync_camera_input),
            ),
        )
        .expect("register editor ImGui UI");
    }
}

fn update_notice(time: Res<Time>, mut notice: ResMut<EditorNotice>) {
    let Some(timer) = notice.expires.as_mut() else {
        return;
    };
    timer.tick(time.delta());
    if timer.is_finished() {
        notice.message.clear();
        notice.expires = None;
    }
}
