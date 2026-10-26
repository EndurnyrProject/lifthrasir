//! Move/rotate/scale handles on the selected nodes, through the Bevy-free
//! `transform-gizmo` core: Bevy feeds it the editor camera and cursor, and
//! egui paints it clipped to the viewport. With several nodes selected the
//! gizmo sits at their median and transforms them together. One drag records
//! one `ComponentEdit<Transform>` per moved node, as a single undo step.

use super::{
    camera::EditorCamera,
    node::{EditorNodeId, KindFlags, LightFlags, NodeKind, find_node, node_kind},
    selection::Selection,
    ui::{
        EditorUiArea, EditorUiSet, ViewportCursor, command_modifier, viewport_has_keyboard,
        viewport_has_pointer,
    },
    undo::{Batch, ComponentEdit, EditOp, record},
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

/// The gizmo, the entities it moves and the modes it shows.
#[derive(Resource, Default)]
pub struct TransformGizmo {
    gizmo: Gizmo,
    targets: Vec<Entity>,
    modes: EnumSet<GizmoMode>,
}

impl TransformGizmo {
    /// True when the pointer is over a handle of the shown gizmo.
    pub fn is_focused(&self) -> bool {
        !self.targets.is_empty() && self.gizmo.is_focused()
    }
}

/// The drag in progress: each moved node and its transform when the drag
/// started.
#[derive(Resource, Default)]
struct Drag(Option<Vec<(EditorNodeId, Transform)>>);

type NodeQuery<'a> = (Entity, &'a EditorNodeId, KindFlags<'a>);

/// The gizmo on the selected nodes. Needs a render app and picking, so the
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
    if command_modifier(&keys) {
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

/// The selected nodes, while the Select tool edits a map.
#[derive(SystemParam)]
struct SelectedNodes<'w, 's> {
    selection: Res<'w, Selection>,
    mode: Res<'w, ToolMode>,
    screen: Res<'w, State<EditorScreen>>,
    nodes: Query<'w, 's, NodeQuery<'static>>,
    lights: Query<'w, 's, LightFlags>,
}

impl SelectedNodes<'_, '_> {
    /// The selected nodes' entities and kinds, the active node first; empty
    /// outside the Select tool.
    fn get(&self) -> Vec<(Entity, NodeKind)> {
        let editing = *self.mode == ToolMode::Select && *self.screen.get() == EditorScreen::Editing;
        if !editing {
            return Vec::new();
        }
        let active = self.selection.active();
        let others = self.selection.nodes().iter().copied();
        active
            .into_iter()
            .chain(others.filter(|&id| Some(id) != active))
            .filter_map(|id| {
                let (entity, _, flags) = self.nodes.iter().find(|node| *node.1 == id)?;
                node_kind(flags, &self.lights).map(|kind| (entity, kind))
            })
            .collect()
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

/// The mode to show for `selected` (the active node first): the chosen one
/// if any node allows it, else the active node's first allowed mode.
fn gizmo_mode(selected: &[(Entity, NodeKind)], chosen: GizmoModeKind) -> Option<GizmoModeKind> {
    let (_, active_kind) = selected.first()?;
    let any_allows = selected
        .iter()
        .any(|(_, kind)| allowed_modes(*kind).contains(&chosen));
    Some(if any_allows {
        chosen
    } else {
        allowed_modes(*active_kind)[0]
    })
}

/// Targets the selected nodes that allow the shown mode (in the Select tool
/// while editing). Frozen during a drag.
fn sync_target(
    drag: Res<Drag>,
    selected: SelectedNodes,
    choice: Res<GizmoChoice>,
    mut gizmo: ResMut<TransformGizmo>,
) {
    if drag.0.is_some() {
        return;
    }
    let selected = selected.get();
    let Some(mode) = gizmo_mode(&selected, choice.0) else {
        gizmo.targets.clear();
        return;
    };
    gizmo.targets = selected
        .iter()
        .filter(|(_, kind)| allowed_modes(*kind).contains(&mode))
        .map(|(entity, _)| *entity)
        .collect();
    gizmo.modes = GizmoMode::all_from_kind(mode);
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
    let started: Vec<_> = gizmo
        .targets
        .iter()
        .filter_map(|&entity| nodes.get(entity).ok())
        .map(|(id, transform)| (*id, *transform))
        .collect();
    drag.0 = (!started.is_empty()).then_some(started);
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
    if state.targets.is_empty() {
        return;
    }
    let Some(config) = input.config(state.modes) else {
        return;
    };
    let (targets, current): (Vec<Entity>, Vec<GizmoTransform>) = state
        .targets
        .iter()
        .filter_map(|&target| Some((target, to_gizmo(transforms.get(target).ok()?))))
        .unzip();
    state.gizmo.update_config(config);
    let Some((_, updated)) = state.gizmo.update(interaction, &current) else {
        return;
    };
    for (&target, result) in targets.iter().zip(&updated) {
        if let Ok(mut transform) = transforms.get_mut(target) {
            *transform = from_gizmo(result);
        }
    }
}

/// Paints the gizmo over the 3D viewport, under floating windows.
fn draw_gizmo(
    mut contexts: EguiContexts,
    area: Res<EditorUiArea>,
    state: Res<TransformGizmo>,
) -> Result {
    let Some(clip) = area.viewport().filter(|_| !state.targets.is_empty()) else {
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
    if mouse.pressed(MouseButton::Left) {
        return;
    }
    let Some(started) = drag.0.take() else {
        return;
    };
    commands.queue(move |world: &mut World| finish_drag(world, started));
}

fn finish_drag(world: &mut World, started: Vec<(EditorNodeId, Transform)>) {
    let mut ops: Vec<Box<dyn EditOp>> = Vec::new();
    for (id, before) in started {
        let Some(node) = find_node(world, id) else {
            continue;
        };
        let after = world.get::<Transform>(node).copied();
        let Some(after) = after.filter(|after| *after != before) else {
            continue;
        };
        after_node_edit(world, node);
        ops.push(Box::new(
            ComponentEdit::new(id, before, after).with_hook(after_node_edit),
        ));
    }
    if !ops.is_empty() {
        record(world, Box::new(Batch(ops)));
    }
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
    fn the_mode_follows_the_choice_unless_no_selected_node_allows_it() {
        let entity = Entity::from_raw_u32(1).unwrap();
        let mixed = [(entity, NodeKind::Sound), (entity, NodeKind::Prop)];

        assert_eq!(gizmo_mode(&mixed, Rotate), Some(Rotate));
        assert_eq!(gizmo_mode(&mixed[..1], Rotate), Some(Translate));
        assert_eq!(
            gizmo_mode(&[(entity, NodeKind::Sun)], Translate),
            Some(Rotate)
        );
        assert_eq!(gizmo_mode(&[], Translate), None);
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
