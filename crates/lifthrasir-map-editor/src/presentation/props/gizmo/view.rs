use bevy::prelude::*;

use super::{GizmoAxis, GizmoMode, PropGizmo};
use crate::{infrastructure::content::LifPropRef, presentation::props::SelectedProp};

#[derive(Component)]
pub(super) struct PropGizmoRoot;

const AXES: [(Vec3, Color); 3] = [
    (Vec3::X, Color::srgb(0.95, 0.1, 0.1)),
    (Vec3::Y, Color::srgb(0.1, 0.95, 0.2)),
    (Vec3::Z, Color::srgb(0.1, 0.35, 1.0)),
];
const AXIS_LENGTH: f32 = 45.0;

pub(super) fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let root = commands
        .spawn((PropGizmoRoot, Visibility::Hidden, Transform::default()))
        .id();
    for (axis, color) in AXES {
        let mesh = meshes.add(Cuboid::new(
            if axis.x != 0.0 { AXIS_LENGTH } else { 4.0 },
            if axis.y != 0.0 { AXIS_LENGTH } else { 4.0 },
            if axis.z != 0.0 { AXIS_LENGTH } else { 4.0 },
        ));
        commands.spawn((
            GizmoAxis { axis },
            Mesh3d(mesh),
            MeshMaterial3d(materials.add(StandardMaterial {
                // Immediate-mode gizmos provide the visuals; these meshes
                // only provide generous picking volumes around each axis.
                base_color: color.with_alpha(0.0),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..default()
            })),
            Pickable::default(),
            Transform::from_translation(axis * (AXIS_LENGTH * 0.5)),
            ChildOf(root),
        ));
    }
}

pub(super) fn sync_prop_gizmo(
    selected: Res<SelectedProp>,
    props: Query<&GlobalTransform, With<LifPropRef>>,
    mut roots: Query<(&mut Transform, &mut Visibility), With<PropGizmoRoot>>,
) {
    let Ok((mut root, mut visibility)) = roots.single_mut() else {
        return;
    };
    let Some(entity) = selected.0 else {
        *visibility = Visibility::Hidden;
        return;
    };
    let Ok(transform) = props.get(entity) else {
        *visibility = Visibility::Hidden;
        return;
    };
    root.translation = transform.translation();
    *visibility = Visibility::Visible;
}

pub(super) fn draw_prop_gizmo(
    mut gizmos: Gizmos,
    selected: Res<SelectedProp>,
    props: Query<&GlobalTransform, With<LifPropRef>>,
    gizmo: Res<PropGizmo>,
) {
    let Some(entity) = selected.0 else { return };
    let Ok(transform) = props.get(entity) else {
        return;
    };
    let origin = transform.translation();
    match gizmo.mode {
        GizmoMode::Translate => {
            for (axis, color) in AXES {
                gizmos.arrow(origin, origin + axis * AXIS_LENGTH, color);
            }
        }
        GizmoMode::Rotate => {
            for (axis, color) in AXES {
                gizmos
                    .circle(
                        Isometry3d::new(origin, Quat::from_rotation_arc(Vec3::Z, axis)),
                        35.0,
                        color,
                    )
                    .resolution(48);
            }
        }
    }
    gizmos.cross(origin, 6.0, Color::WHITE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gizmo_system_queries_can_initialize_together() {
        let mut app = App::new();
        app.init_resource::<SelectedProp>()
            .add_systems(Update, sync_prop_gizmo);
        app.world_mut().spawn((PropGizmoRoot, Transform::default()));
        app.update();
    }
}
