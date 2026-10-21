//! Tool modes and the toolbar that switches between them.

use super::open::EditorScreen;
use crate::shell::{
    gizmo::GizmoChoice,
    ui::{EditorUiArea, EditorUiSet},
};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use transform_gizmo_bevy::config::GizmoModeKind;

/// What a click in the viewport does.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolMode {
    /// Clicks select object nodes.
    #[default]
    Select,
}

/// The tool mode and the toolbar.
pub struct ToolsPlugin;

impl Plugin for ToolsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ToolMode>().add_systems(
            EguiPrimaryContextPass,
            toolbar
                .in_set(EditorUiSet::ToolBar)
                .run_if(in_state(EditorScreen::Editing)),
        );
    }
}

fn toolbar(
    mut contexts: EguiContexts,
    mut area: ResMut<EditorUiArea>,
    mut mode: ResMut<ToolMode>,
    gizmo: Option<ResMut<GizmoChoice>>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    let mut current = *mode;
    let mut choice = gizmo.as_deref().copied();
    area.show(ctx, "toolbar", |ui| {
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut current, ToolMode::Select, "Select");
                let Some(GizmoChoice(kind)) = &mut choice else {
                    return;
                };
                ui.separator();
                ui.selectable_value(kind, GizmoModeKind::Translate, "Move (W)");
                ui.selectable_value(kind, GizmoModeKind::Rotate, "Rotate (E)");
                ui.selectable_value(kind, GizmoModeKind::Scale, "Scale (R)");
            });
        });
    });
    mode.set_if_neq(current);
    if let (Some(mut gizmo), Some(choice)) = (gizmo, choice) {
        gizmo.set_if_neq(choice);
    }
    Ok(())
}
