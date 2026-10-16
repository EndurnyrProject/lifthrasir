use bevy::prelude::*;
use lifthrasir_runtime::world::{PropRenderPlugin, PropSceneContext, WaterRenderPlugin};

#[test]
fn presentation_capabilities_and_prop_context_are_client_independent() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_asset::<AnimationClip>()
        .init_asset::<AnimationGraph>()
        .add_plugins((
            lifthrasir_runtime::RuntimeCorePlugin,
            PropRenderPlugin,
            WaterRenderPlugin,
        ));
    let root = app
        .world_mut()
        .spawn(PropSceneContext {
            no_shade_tint: [0.2, 0.4, 0.6],
        })
        .id();
    app.update();
    assert_eq!(
        app.world()
            .get::<PropSceneContext>(root)
            .unwrap()
            .no_shade_tint,
        [0.2, 0.4, 0.6]
    );
}
