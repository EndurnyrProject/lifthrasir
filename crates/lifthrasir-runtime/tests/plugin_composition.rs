use bevy::{
    animation::AnimationPlugin, gltf::GltfPlugin, mesh::MeshPlugin, prelude::*,
    world_serialization::WorldSerializationPlugin,
};
use lifthrasir_assets::{ContentAssetsPlugin, catalogs::ContentCatalogPlugin};
use lifthrasir_runtime::{ContentAudioPlugin, ContentRuntimePlugins, WorldRenderPlugin};

#[test]
fn silent_runtime_group_starts_and_updates_without_client_resources() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin::default(),
        ImagePlugin::default(),
        MeshPlugin,
        AnimationPlugin,
        WorldSerializationPlugin,
        GltfPlugin::default(),
    ))
    .add_plugins((ContentAssetsPlugin, ContentCatalogPlugin))
    .add_plugins(
        ContentRuntimePlugins
            .build()
            .disable::<ContentAudioPlugin>(),
    );
    app.finish();
    app.cleanup();
    let actor = app
        .world_mut()
        .spawn((
            lifthrasir_runtime::actors::sprites::kinds::SpriteRoot,
            lifthrasir_runtime::actors::MobSprite::default(),
            lifthrasir_runtime::actors::ActorAnimationInput {
                action: lifthrasir_runtime::actors::ActionType::Walk,
                restart_id: 1,
                ..default()
            },
        ))
        .id();
    app.update();
    app.update();
    assert_eq!(
        app.world()
            .get::<lifthrasir_runtime::actors::MobSprite>(actor)
            .unwrap()
            .action_type,
        lifthrasir_runtime::actors::ActionType::Walk
    );
    assert!(app.is_plugin_added::<WorldRenderPlugin>());
    assert!(!app.is_plugin_added::<ContentAudioPlugin>());
    assert!(
        app.world()
            .contains_resource::<Assets<lifthrasir_runtime::world::water::WaterMaterial>>()
    );
}
