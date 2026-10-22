//! Immediate-mode drawing; document/file operations stay in their own modules.
use bevy::prelude::*;
use dear_imgui_bevy::prelude::ImguiFrame;
use dear_imgui_rs::{Condition, Ui, WindowFlags};

use super::{
    EditorUiState,
    files::{MapFilePicker, start_map_file_picker},
};
use crate::{
    application::{EditorDocument, EditorNotice, save},
    presentation::terrain_grid::TerrainGridStyle,
};

pub(super) fn draw(
    frame: ImguiFrame<'_>,
    mut state: ResMut<EditorUiState>,
    mut style: ResMut<TerrainGridStyle>,
    mut picker: ResMut<MapFilePicker>,
    document: Option<ResMut<EditorDocument>>,
    mut notice: ResMut<EditorNotice>,
    mut exit: MessageWriter<AppExit>,
) {
    let ui = frame.ui();
    let mut document = document;
    ui.main_menu_bar(|| {
        ui.menu("File", || {
            if ui.menu_item_with_shortcut("Open File", "Ctrl+O") {
                start_map_file_picker(&mut picker);
            }
            if ui.menu_item_with_shortcut("Save File", "Ctrl+S") {
                match document.as_mut() {
                    Some(document) => save(document, &mut notice),
                    None => notice.show("Open a map before saving"),
                }
            }
            ui.separator();
            if ui.menu_item("Quit") {
                exit.write(AppExit::Success);
            }
        });
        // A menu-bar action, deliberately without a submenu.
        if ui.menu_item("Settings") {
            state.settings_open = true;
        }
    });
    settings_window(ui, &mut state.settings_open, &mut style);
    controls_window(ui);
    notice_window(ui, &notice.message);
}

fn settings_window(ui: &Ui, open: &mut bool, style: &mut ResMut<TerrainGridStyle>) {
    if !*open {
        return;
    }
    ui.window("Settings")
        .opened(open)
        .position([18.0, 42.0], Condition::FirstUseEver)
        .size([380.0, 240.0], Condition::FirstUseEver)
        .build(|| {
            ui.text("Terrain grid");
            ui.separator();
            // Work on copies: borrowing the resource mutably every frame would
            // rebuild the grid even when no widget has actually changed.
            let mut width = style.line_width;
            if ui.slider("Line thickness", 0.02, 0.3, &mut width) {
                style.line_width = width;
            }
            let mut color = style.color;
            if ui.color_edit3("Line color", &mut color) {
                style.color = color;
            }
            ui.text_disabled("Changes apply immediately (this session only).");
        });
}

fn controls_window(ui: &Ui) {
    ui.window("Controls")
        .position(
            [ui.io().display_size()[0] - 340.0, 42.0],
            Condition::FirstUseEver,
        )
        .flags(WindowFlags::ALWAYS_AUTO_RESIZE)
        .build(|| {
            ui.text("Camera");
            ui.text("WASD - move / Space, Left Alt - up, down");
            ui.text("RMB - look / Wheel - speed / P - grid");
            ui.separator();
            ui.text("Collision");
            ui.text("LMB / arrows - select cell");
            ui.text("F - toggle walkability");
            ui.text("Ctrl+Z / Ctrl+Y - undo / redo");
            ui.text("Ctrl+S - save");
            ui.separator();
            ui.text("Props");
            ui.text("Click a prop, drag gizmo axis");
            ui.text("I/K J/L U/O - move / Q/E - rotate");
            ui.text("G/R - gizmo mode");
            ui.text("Ctrl+D - duplicate / Delete - remove");
        });
}

fn notice_window(ui: &Ui, message: &str) {
    if message.is_empty() {
        return;
    }
    ui.window("Notice")
        .position([18.0, ui.io().display_size()[1] - 70.0], Condition::Always)
        .flags(
            WindowFlags::NO_TITLE_BAR
                | WindowFlags::NO_RESIZE
                | WindowFlags::NO_MOVE
                | WindowFlags::ALWAYS_AUTO_RESIZE
                | WindowFlags::NO_INPUTS,
        )
        .build(|| ui.text(message));
}
