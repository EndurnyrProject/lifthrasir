use bevy::{
    gltf::{GltfPlugin, extensions::GltfExtensionHandlers},
    prelude::*,
};
use lifthrasir_runtime::world::{MapGltfPlugin, MapSceneRequest, MapSceneStatus, SceneOwner};

#[test]
fn map_decoder_registers_one_handler_after_gltf_infrastructure() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        GltfPlugin::default(),
    ));
    let before = app
        .world()
        .resource::<GltfExtensionHandlers>()
        .0
        .read_blocking()
        .len();
    app.add_plugins(MapGltfPlugin);
    assert_eq!(
        app.world()
            .resource::<GltfExtensionHandlers>()
            .0
            .read_blocking()
            .len(),
        before + 1
    );
    let owner = app
        .world_mut()
        .spawn((
            MapSceneRequest {
                path: "ro://maps/test/test.glb".into(),
            },
            MapSceneStatus::Loading,
        ))
        .id();
    let child = app.world_mut().spawn(SceneOwner(owner)).id();
    assert_eq!(app.world().get::<SceneOwner>(child).unwrap().0, owner);
}

#[test]
#[should_panic(expected = "MapGltfPlugin requires Bevy glTF infrastructure")]
fn map_decoder_rejects_missing_gltf_infrastructure() {
    App::new().add_plugins(MapGltfPlugin);
}
