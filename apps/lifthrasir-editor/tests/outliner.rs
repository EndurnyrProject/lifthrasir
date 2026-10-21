//! Going to an outliner entry selects the node and points the camera at it.

mod common;

use bevy::prelude::*;
use lifthrasir_editor::shell::{
    camera::EditorCamera, node::EditorNodeId, outliner::go_to, selection::Selection,
};
use lifthrasir_runtime::world::LifAudioEmitter;

#[test]
fn going_to_a_sound_selects_it_and_centres_the_camera_on_it() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    common::open_fixture(&mut app);
    app.update();
    let (sound, id, position) = app
        .world_mut()
        .query_filtered::<(Entity, &EditorNodeId, &GlobalTransform), With<LifAudioEmitter>>()
        .single(app.world())
        .map(|(entity, id, global)| (entity, *id, global.translation()))
        .unwrap();

    go_to(app.world_mut(), sound);

    assert_eq!(app.world().resource::<Selection>().0, Some(id));
    let (camera, transform) = app
        .world_mut()
        .query::<(&EditorCamera, &Transform)>()
        .single(app.world())
        .unwrap();
    assert_eq!(camera.focus, position);
    let to_sound = (position - transform.translation).normalize();
    assert!(transform.forward().abs_diff_eq(to_sound, 1e-4));
}
