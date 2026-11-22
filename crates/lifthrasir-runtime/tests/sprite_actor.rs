use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use lifthrasir_assets::{
    ContentAssetsPlugin,
    animation::{
        ActionData, FrameData, FramePart, PendingAnimation, PendingAnimations, RoAnimationAsset,
        tags::LAYER_BODY,
    },
};
use lifthrasir_runtime::actors::sprites::{
    components::{BodyAttachPoint, HomunculusPartLayer, PendingRenderLayers},
    kinds::SpriteRoot,
};
use lifthrasir_runtime::actors::{
    ActionType, ActorAnimationInput, ActorVisualRoot, EntitySpriteData, EntitySpriteInfo,
    HomunculusSprite, MobSprite, SpawnSpriteEvent,
};
use lifthrasir_runtime::rendering::constants::SPRITE_WORLD_SCALE;
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

#[test]
fn homunculus_spawn_renders_native_attack_groups_through_the_shared_pipeline() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .add_plugins((ContentAssetsPlugin, RuntimeCorePlugin, ActorRenderPlugin))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    let actor = app.world_mut().spawn_empty().id();
    app.world_mut().write_message(SpawnSpriteEvent {
        entity: actor,
        position: Vec3::ZERO,
        sprite_info: EntitySpriteInfo {
            sprite_data: EntitySpriteData::Homunculus {
                sprite_name: "LIF".into(),
            },
        },
    });
    app.update();
    assert!(app.world().get::<HomunculusSprite>(actor).is_some());
    assert!(app.world().get::<MobSprite>(actor).is_none());

    let idle = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::default());
    let attack2 = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::default());
    let frame = |texture_index| FrameData {
        parts: vec![FramePart {
            texture_index,
            transform: Mat4::IDENTITY,
            position: Vec2::ZERO,
            scale: Vec2::ONE,
            texture_size: Vec2::ONE,
            color: Color::WHITE,
            mirror: false,
        }],
        ..default()
    };
    let mut actions = vec![
        ActionData {
            frames: vec![frame(0)],
            delay_ms: 100.0,
        };
        64
    ];
    actions[40] = ActionData {
        frames: vec![frame(1)],
        delay_ms: 100.0,
    };
    let animation = app
        .world_mut()
        .resource_mut::<Assets<RoAnimationAsset>>()
        .add(RoAnimationAsset {
            textures: vec![idle, attack2.clone()],
            actions,
            layer: LAYER_BODY,
            sounds: vec![],
        });
    app.world_mut()
        .entity_mut(actor)
        .insert(ActorAnimationInput {
            action: ActionType::Attack1,
            restart_id: 1,
            ..default()
        });
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
    app.update();

    let sprite = app.world().get::<HomunculusSprite>(actor).unwrap();
    assert_eq!(sprite.action_index(), 40);
    let body = app
        .world_mut()
        .query_filtered::<(Entity, &ActorVisualRoot), With<ActorVisualRoot>>()
        .iter(app.world())
        .find_map(|(body, root)| (root.0 == actor).then_some(body))
        .expect("homunculus body layer");
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
        Some(attack2)
    );
}

#[test]
fn homunculus_body_draws_every_frame_part_and_hides_parts_a_shorter_frame_lacks() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .add_plugins((ContentAssetsPlugin, RuntimeCorePlugin, ActorRenderPlugin))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    let textures: Vec<_> = (0..3)
        .map(|_| {
            app.world_mut()
                .resource_mut::<Assets<Image>>()
                .add(Image::default())
        })
        .collect();
    let part = |texture_index, position, size, mirror| FramePart {
        texture_index,
        transform: Mat4::IDENTITY,
        position,
        scale: Vec2::ONE,
        texture_size: size,
        color: Color::WHITE,
        mirror,
    };
    let full = FrameData {
        parts: vec![
            part(0, Vec2::new(1.0, 2.0), Vec2::new(18.0, 29.0), false),
            part(1, Vec2::new(-3.0, 40.0), Vec2::new(60.0, 71.0), true),
            part(2, Vec2::new(5.0, 6.0), Vec2::new(4.0, 8.0), false),
        ],
        ..default()
    };
    let short = FrameData {
        parts: vec![part(1, Vec2::new(0.0, 30.0), Vec2::new(60.0, 71.0), false)],
        ..default()
    };
    let mut actions = vec![ActionData::default(); 64];
    actions[8] = ActionData {
        frames: vec![full, short],
        delay_ms: 100.0,
    };
    let animation = app
        .world_mut()
        .resource_mut::<Assets<RoAnimationAsset>>()
        .add(RoAnimationAsset {
            textures: textures.clone(),
            actions,
            layer: LAYER_BODY,
            sounds: vec![],
        });
    let actor = app
        .world_mut()
        .spawn((
            SpriteRoot,
            PendingRenderLayers,
            HomunculusSprite::new(animation.clone()),
            ActorAnimationInput {
                action: ActionType::Walk,
                restart_id: 1,
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
    app.update();

    let body = app
        .world_mut()
        .query_filtered::<(Entity, &ActorVisualRoot), With<BodyAttachPoint>>()
        .iter(app.world())
        .find_map(|(body, root)| (root.0 == actor).then_some(body))
        .expect("homunculus body layer");
    let mut parts: Vec<_> = app
        .world_mut()
        .query::<(Entity, &HomunculusPartLayer, &ActorVisualRoot)>()
        .iter(app.world())
        .filter(|(_, _, root)| root.0 == actor)
        .map(|(entity, layer, _)| (layer.part, entity))
        .collect();
    parts.sort();
    assert_eq!(
        parts.iter().map(|(part, _)| *part).collect::<Vec<_>>(),
        vec![1, 2]
    );
    let layers = [body, parts[0].1, parts[1].1];

    let texture = |app: &App, entity| {
        let material = &app
            .world()
            .get::<MeshMaterial3d<StandardMaterial>>(entity)
            .unwrap()
            .0;
        let material = app
            .world()
            .resource::<Assets<StandardMaterial>>()
            .get(material)
            .unwrap();
        (material.base_color_texture.clone(), material.depth_bias)
    };
    for (index, layer) in layers.iter().enumerate() {
        assert_eq!(texture(&app, *layer).0, Some(textures[index].clone()));
    }
    let main = app.world().get::<Transform>(layers[1]).unwrap();
    assert!((main.scale.x + 60.0 * SPRITE_WORLD_SCALE).abs() < 1e-5);
    assert!((main.scale.y - 71.0 * SPRITE_WORLD_SCALE).abs() < 1e-5);
    assert!((main.translation.x + 3.0 * SPRITE_WORLD_SCALE).abs() < 1e-5);
    assert!((main.translation.y - 40.0 * SPRITE_WORLD_SCALE).abs() < 1e-5);
    let z = |app: &App, entity| app.world().get::<Transform>(entity).unwrap().translation.z;
    assert!(z(&app, layers[0]) < z(&app, layers[1]) && z(&app, layers[1]) < z(&app, layers[2]));
    assert!(texture(&app, layers[0]).1 < texture(&app, layers[1]).1);
    assert!(texture(&app, layers[1]).1 < texture(&app, layers[2]).1);
    for layer in &layers[1..] {
        assert_eq!(
            app.world().get::<Visibility>(*layer),
            Some(&Visibility::Inherited)
        );
    }

    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        150,
    )));
    app.update();
    assert_eq!(
        app.world().get::<Visibility>(layers[2]),
        Some(&Visibility::Hidden)
    );
    assert_eq!(
        app.world().get::<Visibility>(layers[1]),
        Some(&Visibility::Hidden)
    );
    assert_eq!(texture(&app, layers[0]).0, Some(textures[1].clone()));
}
