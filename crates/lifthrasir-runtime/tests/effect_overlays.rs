use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use lifthrasir_assets::animation::tags::LAYER_BODY;
use lifthrasir_runtime::actors::RenderLayer;
use lifthrasir_runtime::effects::overlays::{
    AnimationPaused, BodyStateTint, Opt3Tint, SightOrbit, SpiritSphereOrbit, apply_body_state_tint,
    orbit_sight_visuals, orbit_spirit_spheres,
};
use std::time::Duration;

#[test]
fn visual_tints_apply_and_reset_without_gameplay_status_resources() {
    let mut app = App::new();
    app.init_resource::<Assets<StandardMaterial>>()
        .add_systems(Update, apply_body_state_tint);
    let material = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial::default());
    let root = app
        .world_mut()
        .spawn((
            BodyStateTint(Color::srgb(0.5, 0.8, 0.5)),
            Opt3Tint(Color::srgb(0.5, 0.5, 0.25)),
            AnimationPaused { at_ms: 123 },
        ))
        .id();
    app.world_mut().spawn((
        MeshMaterial3d(material.clone()),
        RenderLayer::body(Handle::default(), LAYER_BODY, vec![]),
        ChildOf(root),
    ));
    app.update();
    assert_eq!(
        app.world()
            .resource::<Assets<StandardMaterial>>()
            .get(&material)
            .unwrap()
            .base_color,
        Color::srgb(0.25, 0.4, 0.125)
    );
    app.world_mut()
        .entity_mut(root)
        .remove::<(BodyStateTint, Opt3Tint)>();
    app.update();
    assert_eq!(
        app.world()
            .resource::<Assets<StandardMaterial>>()
            .get(&material)
            .unwrap()
            .base_color,
        Color::WHITE
    );
    assert_eq!(app.world().get::<AnimationPaused>(root).unwrap().at_ms, 123);
}

#[test]
fn orbit_geometry_and_cleanup_need_only_explicit_entity_anchors() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
        .add_systems(Update, (orbit_sight_visuals, orbit_spirit_spheres));
    let unit = app
        .world_mut()
        .spawn(GlobalTransform::from_translation(Vec3::new(
            10.0, 0.0, 20.0,
        )))
        .id();
    let sight = app
        .world_mut()
        .spawn((SightOrbit { unit }, Transform::default()))
        .id();
    let sphere = app
        .world_mut()
        .spawn((
            SpiritSphereOrbit {
                unit,
                index: 0,
                count: 3,
            },
            Transform::default(),
        ))
        .id();
    let unrelated = app.world_mut().spawn_empty().id();
    app.update();
    assert_eq!(
        app.world().get::<Transform>(sight).unwrap().translation,
        Vec3::new(19.0, 8.0, 20.0)
    );
    assert_eq!(
        app.world().get::<Transform>(sphere).unwrap().translation,
        Vec3::new(21.0, 10.0, 20.0)
    );
    app.world_mut().despawn(unit);
    app.update();
    assert!(app.world().get_entity(sight).is_err());
    assert!(app.world().get_entity(sphere).is_err());
    assert!(app.world().get_entity(unrelated).is_ok());
}
