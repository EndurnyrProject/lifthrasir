use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use lifthrasir_assets::{
    ContentAssetsPlugin,
    animation::{
        ActionData, FrameData, FramePart, PendingAnimation, PendingAnimations, RoAnimationAsset,
        tags::LAYER_BODY,
    },
};
use lifthrasir_runtime::actors::sprites::{components::PendingRenderLayers, kinds::SpriteRoot};
use lifthrasir_runtime::actors::{ActionType, ActorAnimationInput, ActorVisualRoot, MobSprite};
use lifthrasir_runtime::{ActorRenderPlugin, RuntimeCorePlugin};
use std::time::Duration;

#[test]
fn non_directional_sprite_roots_keep_their_pose_when_the_camera_turns() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .add_plugins((ContentAssetsPlugin, RuntimeCorePlugin, ActorRenderPlugin));
    app.world_mut().spawn((
        Camera3d::default(),
        Transform::IDENTITY.looking_to(Vec3::new(-0.707, -0.707, 0.0), Vec3::Y),
    ));
    let actor = app
        .world_mut()
        .spawn((SpriteRoot, MobSprite::default()))
        .id();
    app.update();
    assert_eq!(
        app.world().get::<MobSprite>(actor).unwrap().direction,
        lifthrasir_runtime::rendering::Direction::South
    );
    assert!(
        app.world()
            .get::<lifthrasir_runtime::actors::CharacterDirection>(actor)
            .is_none()
    );
}

#[test]
fn synthetic_actor_builds_and_animates_without_client_components() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .add_plugins((ContentAssetsPlugin, RuntimeCorePlugin, ActorRenderPlugin))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    let first = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::default());
    let second = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::default());
    let frame = |texture_index| FrameData {
        parts: vec![FramePart {
            texture_index,
            transform: Mat4::IDENTITY,
            position: Vec2::new(0.0, 10.0),
            scale: Vec2::ONE,
            texture_size: Vec2::new(2.0, 3.0),
            color: Color::WHITE,
            mirror: false,
        }],
        ..default()
    };
    let mut actions = vec![ActionData::default(); 16];
    actions[8] = ActionData {
        frames: vec![frame(0), frame(1)],
        delay_ms: 100.0,
    };
    let animation = app
        .world_mut()
        .resource_mut::<Assets<RoAnimationAsset>>()
        .add(RoAnimationAsset {
            textures: vec![first.clone(), second.clone()],
            actions,
            layer: LAYER_BODY,
            sounds: vec![],
        });
    let actor = app
        .world_mut()
        .spawn((
            SpriteRoot,
            PendingRenderLayers,
            MobSprite::new(animation.clone()),
            ActorAnimationInput {
                action: ActionType::Walk,
                restart_id: 7,
                ..default()
            },
        ))
        .id();
    app.world_mut()
        .resource_mut::<PendingAnimations>()
        .defer_completed(vec![(
            PendingAnimation {
                sprite_handle: Handle::default(),
                action_handle: Handle::default(),
                palette_handle: None,
                layer_tag: LAYER_BODY,
                callback_entity: Some(actor),
            },
            animation,
        )]);
    app.update();
    let meshes: Vec<_> = app
        .world_mut()
        .query_filtered::<Entity, With<ActorVisualRoot>>()
        .iter(app.world())
        .collect();
    assert_eq!(meshes.len(), 1);
    let body = meshes[0];
    assert_eq!(app.world().get::<ActorVisualRoot>(body).unwrap().0, actor);
    let material = app
        .world()
        .get::<MeshMaterial3d<StandardMaterial>>(body)
        .unwrap()
        .0
        .clone();
    assert_eq!(
        app.world()
            .resource::<Assets<StandardMaterial>>()
            .get(&material)
            .unwrap()
            .base_color_texture,
        Some(first)
    );
    assert_eq!(
        app.world()
            .get::<ActorAnimationInput>(actor)
            .unwrap()
            .restart_id,
        7
    );
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        150,
    )));
    app.update();
    assert_eq!(
        app.world()
            .resource::<Assets<StandardMaterial>>()
            .get(&material)
            .unwrap()
            .base_color_texture,
        Some(second)
    );
    assert_eq!(
        app.world().get::<MobSprite>(actor).unwrap().action_type,
        ActionType::Walk
    );
}
