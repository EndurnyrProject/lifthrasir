//! Move/rotate/scale handles on the selected node, through the Bevy-free
//! `transform-gizmo` core: Bevy feeds it the editor camera and cursor, and
//! egui paints it clipped to the viewport. One drag records one
//! `ComponentEdit<Transform>`.

use super::{
    camera::EditorCamera,
    node::{EditorNodeId, KindFlags, LightFlags, NodeKind, find_node, node_kind},
    selection::Selection,
    ui::{EditorUiArea, EditorUiSet, ViewportCursor, viewport_has_keyboard, viewport_has_pointer},
    undo::{ComponentEdit, record},
};
use crate::map::{open::EditorScreen, refresh::after_node_edit, tools::ToolMode};
use bevy::{ecs::system::SystemParam, prelude::*, window::PrimaryWindow};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use transform_gizmo::{
    EnumSet, Gizmo, GizmoConfig, GizmoInteraction, GizmoMode,
    config::GizmoModeKind,
    math::{DMat4, DQuat, DVec3, Pos2, Rect, Transform as GizmoTransform},
};

/// The transform modes a node kind may use; the first is the fallback when
/// the chosen mode is not allowed.
pub fn allowed_modes(kind: NodeKind) -> &'static [GizmoModeKind] {
    use GizmoModeKind::{Rotate, Scale, Translate};
    match kind {
        NodeKind::Prop => &[Translate, Rotate, Scale],
        NodeKind::Sound | NodeKind::Effect | NodeKind::PointLight => &[Translate],
        NodeKind::SpotLight => &[Translate, Rotate],
        NodeKind::Sun => &[Rotate],
    }
}

/// The mode the user picked with 1/2/3 or the toolbar.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct GizmoChoice(pub GizmoModeKind);

impl Default for GizmoChoice {
    fn default() -> Self {
        Self(GizmoModeKind::Translate)
    }
}

/// The gizmo, the entity it moves and the modes it shows.
#[derive(Resource, Default)]
pub struct TransformGizmo {
    gizmo: Gizmo,
    target: Option<Entity>,
    modes: EnumSet<GizmoMode>,
}

impl TransformGizmo {
    /// True when the pointer is over a handle of the shown gizmo.
    pub fn is_focused(&self) -> bool {
        self.target.is_some() && self.gizmo.is_focused()
    }
}

/// The drag in progress: the node and its transform when the drag started.
#[derive(Resource, Default)]
struct Drag(Option<(EditorNodeId, Transform)>);

type NodeQuery<'a> = (Entity, &'a EditorNodeId, KindFlags<'a>);

/// The gizmo on the selected node. Needs a render app and picking, so the
/// binary adds it; headless apps leave it out.
pub struct GizmoPlugin;

impl Plugin for GizmoPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TransformGizmo>()
            .init_resource::<GizmoChoice>()
            .init_resource::<Drag>()
            .add_systems(
                Update,
                (
                    choice_keys.run_if(viewport_has_keyboard),
                    sync_target,
                    start_drag.run_if(viewport_has_pointer),
                    update_gizmo,
                    continue_drag,
                )
                    .chain(),
            )
            .add_systems(
                EguiPrimaryContextPass,
                draw_gizmo.after(EditorUiSet::StatusBar),
            );
    }
}

/// 1/2/3 pick translate/rotate/scale (WASD/Q/E move the camera).
fn choice_keys(keys: Res<ButtonInput<KeyCode>>, mut choice: ResMut<GizmoChoice>) {
    let modifier = keys.any_pressed([
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]);
    if modifier {
        return;
    }
    let picked = [
        (KeyCode::Digit1, GizmoModeKind::Translate),
        (KeyCode::Digit2, GizmoModeKind::Rotate),
        (KeyCode::Digit3, GizmoModeKind::Scale),
    ]
    .into_iter()
    .find(|(key, _)| keys.just_pressed(*key));
    if let Some((_, kind)) = picked {
        choice.set_if_neq(GizmoChoice(kind));
    }
}

/// The selected node, while the Select tool edits a map.
#[derive(SystemParam)]
struct SelectedNode<'w, 's> {
    selection: Res<'w, Selection>,
    mode: Res<'w, ToolMode>,
    screen: Res<'w, State<EditorScreen>>,
    nodes: Query<'w, 's, NodeQuery<'static>>,
    lights: Query<'w, 's, LightFlags>,
}

impl SelectedNode<'_, '_> {
    /// The selected node's entity and kind; `None` outside the Select tool.
    fn get(&self) -> Option<(Entity, NodeKind)> {
        let editing = *self.mode == ToolMode::Select && *self.screen.get() == EditorScreen::Editing;
        self.selection
            .0
            .filter(|_| editing)
            .and_then(|id| self.nodes.iter().find(|node| *node.1 == id))
            .and_then(|(entity, _, flags)| {
                node_kind(flags, &self.lights).map(|kind| (entity, kind))
            })
    }
}

/// The editor camera, cursor and mouse the gizmo is driven by.
#[derive(SystemParam)]
struct GizmoInput<'w, 's> {
    mouse: Res<'w, ButtonInput<MouseButton>>,
    drag: Res<'w, Drag>,
    cursor: ViewportCursor<'w, 's>,
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    cameras: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<EditorCamera>>,
}

impl GizmoInput<'_, '_> {
    /// The gizmo seen through the editor camera, showing `modes`.
    fn config(&self, modes: EnumSet<GizmoMode>) -> Option<GizmoConfig> {
        let window = self.windows.single().ok()?;
        let (camera, global) = self.cameras.single().ok()?;
        let viewport = camera.logical_viewport_rect()?;
        Some(GizmoConfig {
            view_matrix: dmat4(global.to_matrix().inverse()).into(),
            projection_matrix: dmat4(camera.clip_from_view()).into(),
            viewport: Rect::from_min_max(
                Pos2::new(viewport.min.x, viewport.min.y),
                Pos2::new(viewport.max.x, viewport.max.y),
            ),
            modes,
            pixels_per_point: window.scale_factor(),
            ..GizmoConfig::default()
        })
    }

    /// This frame's pointer. `last` keeps the cursor from when it was last
    /// in the window, so a drag survives leaving it.
    fn interaction(&self, last: &mut Vec2) -> GizmoInteraction {
        let window_cursor = self.windows.single().ok().and_then(Window::cursor_position);
        *last = window_cursor.unwrap_or(*last);
        let dragging = self.drag.0.is_some();
        GizmoInteraction {
            cursor_pos: (last.x, last.y),
            hovered: dragging || self.cursor.ray().is_some(),
            drag_started: dragging && self.mouse.just_pressed(MouseButton::Left),
            dragging: dragging && self.mouse.pressed(MouseButton::Left),
        }
    }
}

/// Targets the selected node only (in the Select tool while editing) and
/// restricts the gizmo to the chosen mode, or the node kind's first allowed
/// mode. Frozen during a drag.
fn sync_target(
    drag: Res<Drag>,
    selected: SelectedNode,
    choice: Res<GizmoChoice>,
    mut gizmo: ResMut<TransformGizmo>,
) {
    if drag.0.is_some() {
        return;
    }
    let wanted = selected.get();
    gizmo.target = wanted.map(|(entity, _)| entity);
    let Some((_, kind)) = wanted else {
        return;
    };
    let allowed = allowed_modes(kind);
    let active = if allowed.contains(&choice.0) {
        choice.0
    } else {
        allowed[0]
    };
    gizmo.modes = GizmoMode::all_from_kind(active);
}

/// A left press on the hovered gizmo, over the viewport, starts a drag.
fn start_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    gizmo: Res<TransformGizmo>,
    nodes: Query<(&EditorNodeId, &Transform)>,
    mut drag: ResMut<Drag>,
) {
    if drag.0.is_some() || !mouse.just_pressed(MouseButton::Left) || !gizmo.is_focused() {
        return;
    }
    let Some(Ok((id, transform))) = gizmo.target.map(|entity| nodes.get(entity)) else {
        return;
    };
    drag.0 = Some((*id, *transform));
}

/// Feeds the editor camera and cursor to the gizmo and applies what a drag
/// did to the target's transform.
fn update_gizmo(
    input: GizmoInput,
    mut transforms: Query<&mut Transform>,
    mut state: ResMut<TransformGizmo>,
    mut last_cursor: Local<Vec2>,
) {
    let interaction = input.interaction(&mut last_cursor);
    let Some(target) = state.target else {
        return;
    };
    let (Some(config), Ok(mut transform)) = (input.config(state.modes), transforms.get_mut(target))
    else {
        return;
    };
    state.gizmo.update_config(config);
    let Some((_, updated)) = state.gizmo.update(interaction, &[to_gizmo(&transform)]) else {
        return;
    };
    if let Some(result) = updated.first() {
        *transform = from_gizmo(result);
    }
}

/// Paints the gizmo over the 3D viewport, under floating windows.
fn draw_gizmo(
    mut contexts: EguiContexts,
    area: Res<EditorUiArea>,
    state: Res<TransformGizmo>,
) -> Result {
    let (Some(_), Some(clip)) = (state.target, area.viewport()) else {
        return Ok(());
    };
    let ctx = contexts.ctx_mut()?;
    let data = state.gizmo.draw();
    let vertices = data
        .vertices
        .into_iter()
        .zip(data.colors)
        .map(|(pos, [r, g, b, a])| egui::epaint::Vertex {
            pos: pos.into(),
            uv: egui::Pos2::ZERO,
            color: egui::Rgba::from_rgba_premultiplied(r, g, b, a).into(),
        })
        .collect();
    egui::Painter::new(ctx.clone(), egui::LayerId::background(), clip).add(egui::Mesh {
        indices: data.indices,
        vertices,
        ..default()
    });
    Ok(())
}

/// Keeps the drag going while the button is held; on release records one
/// edit and refreshes what the runtime spawned from the node.
fn continue_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    mut drag: ResMut<Drag>,
    mut commands: Commands,
) {
    let Some((id, before)) = drag.0 else {
        return;
    };
    if mouse.pressed(MouseButton::Left) {
        return;
    }
    drag.0 = None;
    commands.queue(move |world: &mut World| finish_drag(world, id, before));
}

fn finish_drag(world: &mut World, id: EditorNodeId, before: Transform) {
    let Some(node) = find_node(world, id) else {
        return;
    };
    let Some(after) = world.get::<Transform>(node).copied() else {
        return;
    };
    if after == before {
        return;
    }
    after_node_edit(world, node);
    let edit = ComponentEdit::new(id, before, after).with_hook(after_node_edit);
    record(world, Box::new(edit));
}

fn dmat4(matrix: Mat4) -> DMat4 {
    DMat4::from_cols_array(&matrix.to_cols_array().map(f64::from))
}

fn to_gizmo(transform: &Transform) -> GizmoTransform {
    GizmoTransform::from_scale_rotation_translation(
        DVec3::from(transform.scale.to_array().map(f64::from)),
        DQuat::from_array(transform.rotation.to_array().map(f64::from)),
        DVec3::from(transform.translation.to_array().map(f64::from)),
    )
}

fn from_gizmo(transform: &GizmoTransform) -> Transform {
    let vec3 =
        |v: transform_gizmo::mint::Vector3<f64>| Vec3::new(v.x as f32, v.y as f32, v.z as f32);
    let rotation = transform.rotation;
    Transform {
        translation: vec3(transform.translation),
        rotation: Quat::from_xyzw(
            rotation.v.x as f32,
            rotation.v.y as f32,
            rotation.v.z as f32,
            rotation.s as f32,
        ),
        scale: vec3(transform.scale),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use GizmoModeKind::{Rotate, Scale, Translate};

    #[test]
    fn transforms_round_trip_through_the_gizmo() {
        let transform = Transform {
            translation: Vec3::new(1.5, -2.0, 30.0),
            rotation: Quat::from_euler(EulerRot::XYZ, 0.3, -1.2, 2.0),
            scale: Vec3::new(0.5, 2.0, 1.0),
        };

        assert_eq!(from_gizmo(&to_gizmo(&transform)), transform);
    }

    #[test]
    fn each_node_kind_allows_only_its_transform_modes() {
        assert_eq!(
            allowed_modes(NodeKind::Prop),
            &[Translate, Rotate, Scale][..]
        );
        assert_eq!(allowed_modes(NodeKind::Sound), &[Translate][..]);
        assert_eq!(allowed_modes(NodeKind::Effect), &[Translate][..]);
        assert_eq!(allowed_modes(NodeKind::PointLight), &[Translate][..]);
        assert_eq!(allowed_modes(NodeKind::SpotLight), &[Translate, Rotate][..]);
        assert_eq!(allowed_modes(NodeKind::Sun), &[Rotate][..]);
    }
}
