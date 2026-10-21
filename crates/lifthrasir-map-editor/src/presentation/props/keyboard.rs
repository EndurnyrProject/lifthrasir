use bevy::prelude::*;

use super::SelectedProp;
use crate::{
    application::EditorDocument,
    infrastructure::content::{LifMapNodeIndex, LifPropRef},
};

/// Keyboard fine-control remains available alongside the mouse gizmo.
/// I/K move along Z, J/L along X, U/O along Y, and Q/E rotate around Y.
pub(super) fn move_selected_prop(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    selected: Res<SelectedProp>,
    mut document: ResMut<EditorDocument>,
    mut props: Query<(&mut Transform, &LifMapNodeIndex), With<LifPropRef>>,
) {
    let Some(entity) = selected.0 else { return };
    if !keys.any_pressed([
        KeyCode::KeyI,
        KeyCode::KeyK,
        KeyCode::KeyJ,
        KeyCode::KeyL,
        KeyCode::KeyU,
        KeyCode::KeyO,
        KeyCode::KeyQ,
        KeyCode::KeyE,
    ]) {
        return;
    }
    let Ok((mut transform, node)) = props.get_mut(entity) else {
        return;
    };
    let distance = 20.0 * time.delta_secs();
    if keys.pressed(KeyCode::KeyI) {
        transform.translation.z -= distance;
    }
    if keys.pressed(KeyCode::KeyK) {
        transform.translation.z += distance;
    }
    if keys.pressed(KeyCode::KeyJ) {
        transform.translation.x -= distance;
    }
    if keys.pressed(KeyCode::KeyL) {
        transform.translation.x += distance;
    }
    if keys.pressed(KeyCode::KeyU) {
        transform.translation.y += distance;
    }
    if keys.pressed(KeyCode::KeyO) {
        transform.translation.y -= distance;
    }
    if keys.pressed(KeyCode::KeyQ) {
        transform.rotate_y(time.delta_secs());
    }
    if keys.pressed(KeyCode::KeyE) {
        transform.rotate_y(-time.delta_secs());
    }
    if let Err(error) = document.0.update_prop_transform(
        node.0,
        transform.translation.to_array(),
        transform.rotation.to_array(),
        transform.scale.to_array(),
    ) {
        error!("recording keyboard prop transform: {error:#}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;
    use lifthrasir_data::lif::LifProp;
    use std::time::Duration;

    #[test]
    fn keyboard_movement_still_edits_the_document_without_saving() {
        let (_directory, document) = test_support::document();
        let path = document.source_path.clone();
        let original = std::fs::read(&path).unwrap();
        let mut app = App::new();
        app.insert_resource(EditorDocument(document))
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Time>()
            .add_systems(Update, move_selected_prop);
        let prop = app
            .world_mut()
            .spawn((
                LifPropRef(LifProp {
                    model: "ro://models/tree.glb".into(),
                    anim_type: 0,
                    anim_speed: 1.0,
                }),
                LifMapNodeIndex(0),
                Transform::default(),
            ))
            .id();
        app.insert_resource(SelectedProp(Some(prop)));
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs(1));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyL);
        app.update();
        app.update();
        assert_eq!(
            app.world().get::<Transform>(prop).unwrap().translation,
            Vec3::X * 40.0
        );
        let mut document = app.world_mut().resource_mut::<EditorDocument>();
        assert_eq!(document.0.prop_transform(0).unwrap().0, [40.0, 0.0, 0.0]);
        assert!(document.0.undo());
        assert_eq!(document.0.prop_transform(0).unwrap().0, [0.0; 3]);
        assert!(!document.0.undo());
        assert_eq!(std::fs::read(path).unwrap(), original);
    }
}
