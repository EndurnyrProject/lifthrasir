mod gizmo;
mod keyboard;
mod selection;

use bevy::prelude::*;

use crate::{
    application::{EditorDocument, EditorNotice, EditorSystems, control_pressed},
    infrastructure::content::{LifMapNodeIndex, LifPropRef, PropSceneContext},
};

#[derive(Resource, Default)]
struct SelectedProp(Option<Entity>);

pub(super) struct PropEditorPlugin;

impl Plugin for PropEditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SelectedProp>()
            .add_plugins((selection::PropSelectionPlugin, gizmo::PropGizmoPlugin))
            .add_systems(
                Update,
                (
                    keyboard::move_selected_prop,
                    duplicate_selected_prop,
                    delete_selected_prop,
                )
                    .chain()
                    .in_set(EditorSystems::Edit)
                    .run_if(resource_exists::<EditorDocument>),
            )
            .add_systems(
                Update,
                sync_document_props
                    .in_set(EditorSystems::Present)
                    .run_if(resource_exists::<EditorDocument>),
            );
    }
}

fn duplicate_selected_prop(
    keys: Res<ButtonInput<KeyCode>>,
    selected: Res<SelectedProp>,
    mut document: ResMut<EditorDocument>,
    props: Query<(&LifMapNodeIndex, &LifPropRef, &Transform), With<LifPropRef>>,
    mut commands: Commands,
    mut notice: ResMut<EditorNotice>,
) {
    if !keys.just_pressed(KeyCode::KeyD) || !control_pressed(&keys) {
        return;
    }
    let Some(entity) = selected.0 else { return };
    let Ok((node, prop, transform)) = props.get(entity) else {
        return;
    };
    match document.0.duplicate_prop(node.0) {
        Ok(index) => {
            let mut duplicate_transform = *transform;
            duplicate_transform.translation.x += 20.0;
            commands.spawn((
                LifPropRef(prop.0.clone()),
                LifMapNodeIndex(index),
                PropSceneContext::default(),
                duplicate_transform,
            ));
            notice.show(format!("Duplicated prop (node {index}) — Ctrl+S to save"));
        }
        Err(error) => notice.show(format!("Duplicate failed: {error}")),
    }
}

fn delete_selected_prop(
    keys: Res<ButtonInput<KeyCode>>,
    mut document: ResMut<EditorDocument>,
    selected: Res<SelectedProp>,
    mut props: Query<(&LifMapNodeIndex, &mut Visibility), With<LifPropRef>>,
    mut notice: ResMut<EditorNotice>,
) {
    if !keys.just_pressed(KeyCode::Delete) {
        return;
    }
    let Some(entity) = selected.0 else { return };
    let Ok((node, mut visibility)) = props.get_mut(entity) else {
        return;
    };
    if let Err(error) = document.0.set_prop_deleted(node.0, true) {
        error!("deleting prop: {error:#}");
        notice.show(format!("Delete failed: {error}"));
        return;
    }
    *visibility = Visibility::Hidden;
    notice.show("Deleted prop — Ctrl+S to save");
}

/// Scene synchronization belongs to presentation, not the history command.
/// Synchronize every prop so undo does not depend on the current selection.
fn sync_document_props(
    document: Res<EditorDocument>,
    mut props: Query<(&mut Transform, &LifMapNodeIndex, &mut Visibility), With<LifPropRef>>,
) {
    if !document.is_changed() {
        return;
    }
    for (mut transform, node, mut visibility) in &mut props {
        if let Ok((translation, rotation, scale)) = document.0.prop_transform(node.0) {
            transform.translation = Vec3::from(translation);
            transform.rotation = Quat::from_array(rotation);
            transform.scale = Vec3::from(scale);
        }
        *visibility = if document.0.prop_deleted(node.0) {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
}
