//! Tool modes, the toolbar that switches between them, and the terrain point
//! under the cursor that tools act on.

pub mod lights;
pub mod place;

use super::{layout::MapLayout, open::EditorScreen};
use crate::shell::{
    browser::{AssetRef, BrowserSelection},
    gizmo::GizmoChoice,
    outliner::Outliner,
    ui::{EditorUiArea, EditorUiSet, ViewportCursor},
};
use bevy::{
    ecs::system::SystemParam,
    picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings},
    prelude::*,
};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use transform_gizmo_bevy::config::GizmoModeKind;

/// What a click in the viewport does.
#[derive(Resource, Default, Debug, Clone, PartialEq)]
pub enum ToolMode {
    /// Clicks select object nodes.
    #[default]
    Select,
    /// Clicks place the asset on the terrain.
    Place(AssetRef),
    /// Clicks add a light above the terrain.
    AddLight(lights::LightKind),
}

/// The world-space terrain point under the primary window's cursor.
#[derive(SystemParam)]
pub struct TerrainCursor<'w, 's> {
    cursor: ViewportCursor<'w, 's>,
    ray_cast: MeshRayCast<'w, 's>,
    parents: Query<'w, 's, &'static ChildOf>,
    layout: Option<Res<'w, MapLayout>>,
}

impl TerrainCursor<'_, '_> {
    /// The nearest terrain hit under the cursor; `None` over egui panels, off
    /// the terrain, or without an open map. Takes `&mut self` because
    /// `MeshRayCast::cast_ray` does.
    pub fn hit(&mut self) -> Option<Vec3> {
        let terrain = self.layout.as_ref()?.terrain_node?;
        let (_, ray) = self.cursor.ray()?;
        let parents = &self.parents;
        let on_terrain = |entity| {
            parents
                .get(entity)
                .is_ok_and(|parent| parent.parent() == terrain)
        };
        let settings = MeshRayCastSettings::default().with_filter(&on_terrain);
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
            .add_plugins((place::plugin, lights::plugin))
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
    mut outliner: ResMut<Outliner>,
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
                ui.separator();
                for (kind, label) in [
                    (lights::LightKind::Point, "Add point light"),
                    (lights::LightKind::Spot, "Add spot light"),
                ] {
                    ui.selectable_value(&mut current, ToolMode::AddLight(kind), label);
                }
                ui.separator();
                ui.toggle_value(&mut outliner.open, "Outliner");
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
