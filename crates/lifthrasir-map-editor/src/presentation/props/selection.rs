use bevy::prelude::*;

use super::SelectedProp;
use crate::{application::EditorSystems, infrastructure::content::LifPropRef};

#[derive(Component)]
struct PropCursor;

pub(super) struct PropSelectionPlugin;

impl Plugin for PropSelectionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_observer(select_prop)
            .add_systems(Update, sync_prop_cursor.in_set(EditorSystems::Present));
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        PropCursor,
        Visibility::Hidden,
        Mesh3d(meshes.add(Cuboid::new(12.0, 12.0, 12.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.9, 0.05, 0.45),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })),
        Transform::default(),
    ));
}

/// Meshes are nested beneath the map prop, so walk up the owning hierarchy.
fn select_prop(
    click: On<Pointer<Click>>,
    parents: Query<&ChildOf>,
    props: Query<(), With<LifPropRef>>,
    mut selected: ResMut<SelectedProp>,
    capture: Option<Res<dear_imgui_bevy::prelude::ImguiInputCapture>>,
) {
    if crate::presentation::ui::pointer_captured(capture.as_deref()) {
        return;
    }
    let mut entity = click.entity;
    loop {
        if props.contains(entity) {
            selected.0 = Some(entity);
            return;
        }
        let Ok(parent) = parents.get(entity) else {
            return;
        };
        entity = parent.parent();
    }
}

fn sync_prop_cursor(
    selected: Res<SelectedProp>,
    props: Query<&GlobalTransform, With<LifPropRef>>,
    mut cursors: Query<(&mut Transform, &mut Visibility), With<PropCursor>>,
) {
    let Ok((mut cursor, mut visibility)) = cursors.single_mut() else {
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
    cursor.translation = transform.translation();
    *visibility = Visibility::Visible;
}
