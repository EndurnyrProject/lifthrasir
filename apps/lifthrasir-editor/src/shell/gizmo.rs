//! Move/rotate/scale handles on the selected node, through
//! `transform-gizmo-bevy`. One drag records one `ComponentEdit<Transform>`.

use super::{
    camera::{CameraMode, EditorCamera},
    node::{EditorNodeId, find_node},
    selection::Selection,
    ui::{viewport_has_keyboard, viewport_has_pointer},
    undo::{ComponentEdit, record},
};
use crate::map::{open::EditorScreen, refresh::after_node_edit, tools::ToolMode};
use bevy::prelude::*;
use lifthrasir_runtime::world::{LifAudioEmitter, LifEffectEmitter, LifPropRef};
use transform_gizmo_bevy::{
    GizmoCamera, GizmoDragStarted, GizmoDragging, GizmoMode, GizmoOptions, GizmoTarget,
    TransformGizmoPlugin, config::GizmoModeKind,
};

/// What kind of object node a gizmo is attached to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Prop,
    Sound,
    Effect,
    PointLight,
    SpotLight,
    Sun,
}

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

/// The mode the user picked with W/E/R or the toolbar.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct GizmoChoice(pub GizmoModeKind);

impl Default for GizmoChoice {
    fn default() -> Self {
        Self(GizmoModeKind::Translate)
    }
}

/// The drag in progress: the node and its transform when the drag started.
#[derive(Resource, Default)]
struct Drag(Option<(EditorNodeId, Transform)>);

type NodeQuery<'a> = (
    Entity,
    &'a EditorNodeId,
    Has<LifPropRef>,
    Has<LifAudioEmitter>,
    Has<LifEffectEmitter>,
    Option<&'a Children>,
);

/// The gizmo on the selected node. Needs a render app and picking, so the
/// binary adds it; headless apps leave it out.
pub struct GizmoPlugin;

impl Plugin for GizmoPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TransformGizmoPlugin)
            .init_resource::<GizmoChoice>()
            .init_resource::<Drag>()
            .add_observer(mark_gizmo_camera)
            .add_systems(
                Update,
                (
                    choice_keys.run_if(viewport_has_keyboard),
                    sync_target,
                    start_drag.run_if(viewport_has_pointer),
                    continue_drag,
                )
                    .chain(),
            );
    }
}

fn mark_gizmo_camera(added: On<Add, EditorCamera>, mut commands: Commands) {
    commands.entity(added.entity).insert(GizmoCamera);
}

/// W/E/R pick translate/rotate/scale, in Orbit mode only (Fly mode moves
/// with W and E).
fn choice_keys(
    keys: Res<ButtonInput<KeyCode>>,
    cameras: Query<&EditorCamera>,
    mut choice: ResMut<GizmoChoice>,
) {
    let orbit = cameras
        .single()
        .is_ok_and(|camera| camera.mode == CameraMode::Orbit);
    let modifier = keys.any_pressed([
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]);
    if !orbit || modifier {
        return;
    }
    let picked = [
        (KeyCode::KeyW, GizmoModeKind::Translate),
        (KeyCode::KeyE, GizmoModeKind::Rotate),
        (KeyCode::KeyR, GizmoModeKind::Scale),
    ]
    .into_iter()
    .find(|(key, _)| keys.just_pressed(*key));
    if let Some((_, kind)) = picked {
        choice.set_if_neq(GizmoChoice(kind));
    }
}

fn node_kind(
    (prop, sound, effect, children): (bool, bool, bool, Option<&Children>),
    lights: &Query<(Has<DirectionalLight>, Has<SpotLight>, Has<PointLight>)>,
) -> Option<NodeKind> {
    if prop {
        return Some(NodeKind::Prop);
    }
    if sound {
        return Some(NodeKind::Sound);
    }
    if effect {
        return Some(NodeKind::Effect);
    }
    children?
        .iter()
        .find_map(|child| match lights.get(child).ok()? {
            (true, _, _) => Some(NodeKind::Sun),
            (_, true, _) => Some(NodeKind::SpotLight),
            (_, _, true) => Some(NodeKind::PointLight),
            _ => None,
        })
}

/// Keeps `GizmoTarget` on the selected node only (in the Select tool while
/// editing) and restricts the gizmo to the chosen mode, or the node kind's
/// first allowed mode. Frozen during a drag.
#[allow(clippy::too_many_arguments)]
fn sync_target(
    mut commands: Commands,
    drag: Res<Drag>,
    selection: Res<Selection>,
    mode: Res<ToolMode>,
    screen: Res<State<EditorScreen>>,
    choice: Res<GizmoChoice>,
    nodes: Query<NodeQuery<'static>>,
    lights: Query<(Has<DirectionalLight>, Has<SpotLight>, Has<PointLight>)>,
    targets: Query<Entity, With<GizmoTarget>>,
    mut options: ResMut<GizmoOptions>,
) {
    if drag.0.is_some() {
        return;
    }
    let editing = *mode == ToolMode::Select && *screen.get() == EditorScreen::Editing;
    let wanted = selection
        .0
        .filter(|_| editing)
        .and_then(|id| nodes.iter().find(|node| *node.1 == id))
        .and_then(|(entity, _, prop, sound, effect, children)| {
            node_kind((prop, sound, effect, children), &lights).map(|kind| (entity, kind))
        });
    for target in &targets {
        if wanted.is_none_or(|(entity, _)| entity != target) {
            commands.entity(target).remove::<GizmoTarget>();
        }
    }
    let Some((entity, kind)) = wanted else {
        return;
    };
    if !targets.contains(entity) {
        commands.entity(entity).insert(GizmoTarget::default());
    }
    let allowed = allowed_modes(kind);
    let active = if allowed.contains(&choice.0) {
        choice.0
    } else {
        allowed[0]
    };
    let modes = GizmoMode::all_from_kind(active);
    if options.gizmo_modes != modes {
        options.gizmo_modes = modes;
    }
}

/// A left press on the hovered gizmo, over the viewport, starts a drag.
fn start_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    targets: Query<(&EditorNodeId, &Transform, &GizmoTarget)>,
    mut drag: ResMut<Drag>,
    mut started: MessageWriter<GizmoDragStarted>,
) {
    if drag.0.is_some() || !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some((id, transform, _)) = targets.iter().find(|(.., target)| target.is_focused()) else {
        return;
    };
    drag.0 = Some((*id, *transform));
    started.write_default();
}

/// Keeps the drag going while the button is held; on release records one
/// edit and refreshes what the runtime spawned from the node.
fn continue_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    mut drag: ResMut<Drag>,
    mut dragging: MessageWriter<GizmoDragging>,
    mut commands: Commands,
) {
    let Some((id, before)) = drag.0 else {
        return;
    };
    if mouse.pressed(MouseButton::Left) {
        dragging.write_default();
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

#[cfg(test)]
mod tests {
    use super::*;
    use GizmoModeKind::{Rotate, Scale, Translate};

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
