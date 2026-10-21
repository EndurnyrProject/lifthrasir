//! Editor UI ordering, the shared panel area, the status bar, and the run
//! conditions that keep viewport input away from egui.

use super::{camera::EditorCamera, undo::UndoStack};
use crate::{
    EditorPaths,
    map::{open::OpenMap, save::map_glb_path},
};
use bevy::{prelude::*, window::PrimaryWindow};
use bevy_egui::{EguiContexts, egui, input::EguiWantsInput};

/// Editor egui systems, chained in this order in `EguiPrimaryContextPass`.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditorUiSet {
    MenuBar,
    ToolBar,
    LeftPanel,
    RightPanel,
    Windows,
    StatusBar,
}

/// The screen area egui panels have not claimed yet.
///
/// egui 0.36 draws panels inside a `Ui`, not on the screen, and panels on the
/// background layer do not count as "pointer over egui". The editor's panel
/// systems live in separate sets, so they share the leftover rect here; what
/// remains after the last panel is the 3D viewport.
#[derive(Resource, Default)]
pub struct EditorUiArea {
    pass: u64,
    rect: Option<egui::Rect>,
}

impl EditorUiArea {
    /// Runs `add` (typically one `egui::Panel`) in a `Ui` over the area earlier
    /// panels left this pass, then keeps what `add` did not claim.
    pub fn show(&mut self, ctx: &egui::Context, id: &str, add: impl FnOnce(&mut egui::Ui)) {
        let pass = ctx.cumulative_pass_nr();
        let rect = match self.rect {
            Some(rect) if self.pass == pass => rect,
            _ => ctx.viewport_rect(),
        };
        let mut ui = egui::Ui::new(
            ctx.clone(),
            egui::Id::new(id),
            egui::UiBuilder::new()
                .layer_id(egui::LayerId::background())
                .max_rect(rect),
        );
        add(&mut ui);
        self.pass = pass;
        self.rect = Some(ui.available_rect_before_wrap());
    }
}

/// True when the pointer belongs to the 3D viewport: egui does not want it and
/// it is outside every editor panel. Without egui (headless) the viewport owns it.
pub fn viewport_has_pointer(
    egui: Option<Res<EguiWantsInput>>,
    area: Option<Res<EditorUiArea>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) -> bool {
    let cursor = windows.single().ok().and_then(Window::cursor_position);
    viewport_owns_pointer(
        egui.is_some_and(|egui| egui.wants_any_pointer_input()),
        area.and_then(|area| area.rect),
        cursor,
    )
}

/// True when egui is not taking keyboard input. Without egui the viewport owns it.
pub fn viewport_has_keyboard(egui: Option<Res<EguiWantsInput>>) -> bool {
    !egui.is_some_and(|egui| egui.wants_any_keyboard_input())
}

// NOTE: compares egui points with logical window pixels, equal at egui zoom 1;
// convert with the egui zoom factor if editor zoom is ever exposed.
fn viewport_owns_pointer(
    egui_wants: bool,
    viewport: Option<egui::Rect>,
    cursor: Option<Vec2>,
) -> bool {
    if egui_wants {
        return false;
    }
    match (viewport, cursor) {
        (Some(rect), Some(cursor)) => rect.contains(egui::pos2(cursor.x, cursor.y)),
        _ => true,
    }
}

/// Bottom status bar: the open map (`*` when unsaved, a note when it is only
/// in the pak) and the camera mode.
pub fn status_bar(
    mut contexts: EguiContexts,
    mut area: ResMut<EditorUiArea>,
    cameras: Query<&EditorCamera>,
    open: Option<Res<OpenMap>>,
    undo: Res<UndoStack>,
    paths: Option<Res<EditorPaths>>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    let mode = cameras.single().map(|camera| camera.mode).ok();
    let pak_only = match (&open, &paths) {
        (Some(open), Some(paths)) => !map_glb_path(&paths.data_folder, &open.name).exists(),
        _ => false,
    };
    area.show(ctx, "status_bar", |ui| {
        egui::Panel::bottom("status_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(open) = &open {
                    let dirty = if undo.is_dirty() { "*" } else { "" };
                    ui.label(format!("{}{dirty}", open.name));
                }
                if pak_only {
                    ui.label("(pak only: saving writes a loose copy that shadows the pak)");
                }
                if let Some(mode) = mode {
                    ui.separator();
                    ui.label(format!("Camera: {mode:?} (C to toggle)"));
                }
            });
        });
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pointer_belongs_to_the_viewport_only_outside_panels_and_egui() {
        let viewport = Some(egui::Rect::from_min_max(
            egui::pos2(200.0, 0.0),
            egui::pos2(800.0, 580.0),
        ));

        assert!(viewport_owns_pointer(
            false,
            viewport,
            Some(Vec2::new(400.0, 300.0))
        ));
        assert!(!viewport_owns_pointer(
            false,
            viewport,
            Some(Vec2::new(100.0, 300.0))
        ));
        assert!(!viewport_owns_pointer(
            true,
            viewport,
            Some(Vec2::new(400.0, 300.0))
        ));
        assert!(viewport_owns_pointer(
            false,
            None,
            Some(Vec2::new(100.0, 300.0))
        ));
    }
}
