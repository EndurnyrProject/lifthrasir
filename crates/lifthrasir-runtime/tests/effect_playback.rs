use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use lifthrasir_assets::{
    ContentAssetsPlugin,
    effects::{EffectBlend, LoadedEffectAsset, LoadedFrame, LoadedLayer},
};
use lifthrasir_runtime::effects::{ActiveEffect, EffectAnchor, EffectLayer, spawn_effect};
use lifthrasir_runtime::{EffectsPlaybackPlugin, RuntimeCorePlugin};
use std::time::Duration;

#[test]
fn playback_initializes_late_follows_its_anchor_and_finishes_without_client_state() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .add_plugins((
            ContentAssetsPlugin,
            RuntimeCorePlugin,
            EffectsPlaybackPlugin,
        ))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            100,
        )));
    app.finish();
    app.cleanup();
    let handle = app
        .world()
        .resource::<Assets<LoadedEffectAsset>>()
        .reserve_handle();
    let target = app
        .world_mut()
        .spawn(GlobalTransform::from_translation(Vec3::new(5.0, 6.0, 7.0)))
        .id();
    let root = {
        let mut commands = app.world_mut().commands();
        spawn_effect(
            &mut commands,
            handle.clone(),
            EffectAnchor::Entity(target),
            false,
            Color::WHITE,
            None,
        )
    };
    app.world_mut().flush();
    app.update();
    app.update();
    assert_eq!(
        app.world().get::<ActiveEffect>(root).unwrap().timer.elapsed,
        0.0
    );
    app.world_mut()
        .resource_mut::<Assets<LoadedEffectAsset>>()
        .insert(
            &handle,
            LoadedEffectAsset {
                fps: 10,
                max_key: 4,
                layers: vec![LoadedLayer {
                    textures: vec![],
                    frame_index_map: vec![Some(0); 4],
                    blend: EffectBlend::Add,
                    frames: vec![LoadedFrame {
                        frame_index: 0,
                        offset: Vec2::new(319.0, 291.0),
                        xy: [-1.0, 1.0, 1.0, -1.0, -1.0, -1.0, 1.0, 1.0],
                        uv: [0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0],
                        texture_index: 0,
                        color: [255.0; 4],
                        angle: 0.0,
                        blend: EffectBlend::Add,
                    }],
                }],
            },
        )
        .unwrap();
    app.update();
    assert!(
        app.world()
            .get::<ActiveEffect>(root)
            .unwrap()
            .layers_initialized
    );
    assert_eq!(
        app.world_mut()
            .query::<&EffectLayer>()
            .iter(app.world())
            .count(),
        1
    );
    assert_eq!(
        app.world().get::<Transform>(root).unwrap().translation,
        Vec3::new(5.0, 6.0, 7.0)
    );
    *app.world_mut().get_mut::<GlobalTransform>(target).unwrap() =
        GlobalTransform::from_translation(Vec3::new(8.0, 9.0, 10.0));
    app.update();
    assert_eq!(
        app.world().get::<Transform>(root).unwrap().translation,
        Vec3::new(8.0, 9.0, 10.0)
    );
    assert_eq!(
        app.world()
            .get::<ActiveEffect>(root)
            .unwrap()
            .timer
            .current_frame,
        1
    );
    for _ in 0..5 {
        app.update();
    }
    assert!(app.world().get_entity(root).is_err());
    assert_eq!(
        app.world_mut()
            .query::<&EffectLayer>()
            .iter(app.world())
            .count(),
        0
    );
}
