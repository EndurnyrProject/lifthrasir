//! Tool modes, the toolbar that switches between them, and the terrain point
//! under the cursor that tools act on.

pub mod place;

use super::{layout::MapLayout, open::EditorScreen};
use crate::shell::{
    browser::{AssetRef, BrowserSelection},
    camera::EditorCamera,
    gizmo::GizmoChoice,
    ui::{EditorUiArea, EditorUiSet, viewport_owns_pointer},
};
use bevy::{
    ecs::system::SystemParam,
    picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings},
    prelude::*,
    window::PrimaryWindow,
};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui, input::EguiWantsInput};
use transform_gizmo_bevy::config::GizmoModeKind;

/// What a click in the viewport does.
#[derive(Resource, Default, Debug, Clone, PartialEq)]
pub enum ToolMode {
    /// Clicks select object nodes.
    #[default]
    Select,
    /// Clicks place the asset on the terrain.
    Place(AssetRef),
}

/// The world-space terrain point under the primary window's cursor.
#[derive(SystemParam)]
pub struct TerrainCursor<'w, 's> {
    ray_cast: MeshRayCast<'w, 's>,
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    cameras: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<EditorCamera>>,
    parents: Query<'w, 's, &'static ChildOf>,
    layout: Option<Res<'w, MapLayout>>,
    egui: Option<Res<'w, EguiWantsInput>>,
    area: Option<Res<'w, EditorUiArea>>,
}

impl TerrainCursor<'_, '_> {
    /// The nearest terrain hit under the cursor; `None` over egui panels, off
    /// the terrain, or without an open map. Takes `&mut self` because
    /// `MeshRayCast::cast_ray` does.
    pub fn hit(&mut self) -> Option<Vec3> {
        let terrain = self.layout.as_ref()?.terrain_node?;
        let cursor = self.windows.single().ok()?.cursor_position()?;
        let egui_wants = self
            .egui
            .as_ref()
            .is_some_and(|egui| egui.wants_any_pointer_input());
        let viewport = self.area.as_ref().and_then(|area| area.viewport());
        if !viewport_owns_pointer(egui_wants, viewport, Some(cursor)) {
            return None;
        }
        let (camera, global) = self.cameras.single().ok()?;
        let ray = camera.viewport_to_world(global, cursor).ok()?;
        let parents = &self.parents;
        let on_terrain = |entity| {
            parents
                .get(entity)
                .is_ok_and(|parent| parent.parent() == terrain)
        };
        let settings = MeshRayCastSettings::default()
            .with_filter(&on_terrain)
            .with_early_exit_test(&|_| false);
        self.ray_cast
            .cast_ray(ray, &settings)
            .first()
            .map(|(_, hit)| hit.point)
    }
}

/// The tool mode and the toolbar.
pub struct ToolsPlugin;

impl Plugin for ToolsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ToolMode>()
            .add_plugins(place::plugin)
            .add_systems(
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
    browser: Res<BrowserSelection>,
    gizmo: Option<ResMut<GizmoChoice>>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    let mut current = mode.clone();
    let placeable = browser.0.clone().filter(place::is_placeable);
    let mut choice = gizmo.as_deref().copied();
    area.show(ctx, "toolbar", |ui| {
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut current, ToolMode::Select, "Select");
                let placing = matches!(current, ToolMode::Place(_));
                let place = ui
                    .add_enabled(
                        placeable.is_some(),
                        egui::Button::selectable(placing, "Place"),
                    )
                    .on_disabled_hover_text("select a model, sound or effect in the browser");
                if place.clicked()
                    && let Some(asset) = &placeable
                {
                    current = ToolMode::Place(asset.clone());
                }
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
