//! Tool modes and the toolbar that switches between them.

use super::open::EditorScreen;
use crate::shell::ui::{EditorUiArea, EditorUiSet};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};

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
) -> Result {
    let ctx = contexts.ctx_mut()?;
    let mut current = *mode;
    area.show(ctx, "toolbar", |ui| {
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut current, ToolMode::Select, "Select");
            });
        });
    });
    mode.set_if_neq(current);
    Ok(())
}
