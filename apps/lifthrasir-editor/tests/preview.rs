//! Selecting a model in the browser spawns one editor-only preview root on
//! the preview layer; selecting anything else removes it.

mod common;

use bevy::{app::Propagate, camera::visibility::RenderLayers, prelude::*};
use lifthrasir_editor::{
    EditorOnly,
    shell::browser::{AssetRef, BrowserSelection},
};
use lifthrasir_runtime::{rendering::EquipmentPreviewCamera, world::LifPropRef};

fn preview_roots(app: &mut App) -> Vec<(String, RenderLayers)> {
    app.world_mut()
        .query_filtered::<(&LifPropRef, &Propagate<RenderLayers>), With<EditorOnly>>()
        .iter(app.world())
        .map(|(prop, layers)| (prop.0.model.clone(), layers.0.clone()))
        .collect()
}

fn select(app: &mut App, asset: AssetRef) {
    app.world_mut().resource_mut::<BrowserSelection>().0 = Some(asset);
    app.update();
}

#[test]
fn a_selected_model_gets_one_preview_root_until_something_else_is_selected() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    app.update();

    select(&mut app, AssetRef::Model("ro://models/a.glb".into()));
    select(&mut app, AssetRef::Model("ro://models/b.glb".into()));

    assert_eq!(
        preview_roots(&mut app),
        [("ro://models/b.glb".to_string(), RenderLayers::layer(1))]
    );

    select(
        &mut app,
        AssetRef::GroundTexture("ro://ground/tex/grass01.ktx2".into()),
    );

    assert!(preview_roots(&mut app).is_empty());
}

/// Runtime billboard and effect-ordering systems resolve "the" world camera
/// with `single()` and silently skip when there is more than one.
#[test]
fn the_preview_camera_is_not_a_world_camera() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    app.update();

    let world_cameras = app
        .world_mut()
        .query_filtered::<(), (With<Camera3d>, Without<EquipmentPreviewCamera>)>()
        .iter(app.world())
        .count();

    assert_eq!(world_cameras, 1);
}
