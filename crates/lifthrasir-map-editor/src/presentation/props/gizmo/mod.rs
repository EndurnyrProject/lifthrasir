mod view;

use bevy::prelude::*;

use super::SelectedProp;
use crate::{
    application::{EditorDocument, EditorSystems},
    infrastructure::content::{LifMapNodeIndex, LifPropRef},
    presentation::viewport::EditorCamera,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum GizmoMode {
    #[default]
    Translate,
    Rotate,
}

#[derive(Resource, Default)]
struct PropGizmo {
    mode: GizmoMode,
    drag: Option<(Entity, Entity)>,
}

#[derive(Component, Clone, Copy)]
struct GizmoAxis {
    axis: Vec3,
}

pub(super) struct PropGizmoPlugin;

impl Plugin for PropGizmoPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PropGizmo>()
            .add_systems(Startup, view::setup)
            .add_systems(Update, switch_gizmo_mode.in_set(EditorSystems::Edit))
            .add_systems(
                Update,
                (view::sync_prop_gizmo, view::draw_prop_gizmo).in_set(EditorSystems::Present),
            )
            .add_observer(gizmo_drag_start)
            .add_observer(gizmo_drag)
            .add_observer(gizmo_drag_end);
    }
}

fn switch_gizmo_mode(keys: Res<ButtonInput<KeyCode>>, mut gizmo: ResMut<PropGizmo>) {
    if keys.just_pressed(KeyCode::KeyG) {
        gizmo.mode = GizmoMode::Translate;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        gizmo.mode = GizmoMode::Rotate;
    }
}

fn gizmo_drag_start(
    drag: On<Pointer<DragStart>>,
    axes: Query<&GizmoAxis>,
    selected: Res<SelectedProp>,
    mut state: ResMut<PropGizmo>,
) {
    if drag.button != PointerButton::Primary {
        return;
    }
    let Ok(_) = axes.get(drag.entity) else { return };
    let Some(prop) = selected.0 else { return };
    state.drag = Some((drag.entity, prop));
}

fn gizmo_drag(
    drag: On<Pointer<Drag>>,
    axes: Query<&GizmoAxis>,
    cameras: Query<&GlobalTransform, With<EditorCamera>>,
    mut props: Query<(&mut Transform, &LifMapNodeIndex), With<LifPropRef>>,
    mut document: ResMut<EditorDocument>,
    state: Res<PropGizmo>,
) {
    if drag.button != PointerButton::Primary {
        return;
    }
    let Some((axis_entity, prop_entity)) = state.drag else {
        return;
    };
    if axis_entity != drag.entity {
        return;
    }
    let Ok(axis) = axes.get(drag.entity) else {
        return;
    };
    let Ok(camera) = cameras.single() else { return };
    let Ok((mut transform, node)) = props.get_mut(prop_entity) else {
        return;
    };
    let axis = axis.axis;
    let right = camera.right().as_vec3();
    let up = camera.up().as_vec3();
    let screen_axis = Vec2::new(axis.dot(right), -axis.dot(up));
    let amount = if screen_axis.length_squared() > 0.001 {
        drag.delta.dot(screen_axis.normalize()) * 0.08
    } else {
        0.0
    };
    if state.mode == GizmoMode::Translate {
        transform.translation += axis * amount;
    } else {
        transform.rotation = Quat::from_axis_angle(axis, amount * 0.025) * transform.rotation;
    }
    if let Err(error) = document.0.update_prop_transform(
        node.0,
        transform.translation.to_array(),
        transform.rotation.to_array(),
        transform.scale.to_array(),
    ) {
        error!("recording gizmo transform: {error:#}");
    }
}

fn gizmo_drag_end(
    drag: On<Pointer<DragEnd>>,
    mut state: ResMut<PropGizmo>,
    mut document: ResMut<EditorDocument>,
) {
    if drag.button != PointerButton::Primary {
        return;
    }
    let Some((axis, _)) = state.drag else { return };
    if axis != drag.entity {
        return;
    }
    state.drag = None;
    document.0.commit_pending_prop_transform();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;
    use bevy::{
        camera::NormalizedRenderTarget,
        picking::pointer::{Location, PointerId},
        window::WindowRef,
    };

    #[test]
    fn release_coalesces_history_without_saving_and_ignores_right_button() {
        let (_directory, mut document) = test_support::document();
        let path = document.source_path.clone();
        let bytes = std::fs::read(&path).unwrap();
        for x in [1.0, 2.0] {
            document
                .update_prop_transform(0, [x, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0], [1.0; 3])
                .unwrap();
        }
        let mut app = App::new();
        let axis = app.world_mut().spawn_empty().id();
        let prop = app.world_mut().spawn_empty().id();
        let window = app.world_mut().spawn_empty().id();
        app.insert_resource(EditorDocument(document))
            .insert_resource(PropGizmo {
                mode: GizmoMode::Translate,
                drag: Some((axis, prop)),
            })
            .add_observer(gizmo_drag_end);
        for button in [PointerButton::Secondary, PointerButton::Primary] {
            app.world_mut().trigger(Pointer::new(
                PointerId::Mouse,
                Location {
                    target: NormalizedRenderTarget::Window(
                        WindowRef::Primary.normalize(Some(window)).unwrap(),
                    ),
                    position: Vec2::ZERO,
                },
                DragEnd {
                    button,
                    distance: Vec2::ONE,
                },
                axis,
            ));
            if button == PointerButton::Secondary {
                assert!(app.world().resource::<PropGizmo>().drag.is_some());
                assert!(
                    app.world()
                        .resource::<EditorDocument>()
                        .0
                        .has_pending_prop_transform()
                );
            }
        }
        assert!(app.world().resource::<PropGizmo>().drag.is_none());
        let mut document = app.world_mut().resource_mut::<EditorDocument>();
        assert!(!document.0.has_pending_prop_transform());
        assert!(document.0.undo());
        assert_eq!(document.0.prop_transform(0).unwrap().0, [0.0; 3]);
        assert!(!document.0.undo(), "one undo entry per drag");
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
}
