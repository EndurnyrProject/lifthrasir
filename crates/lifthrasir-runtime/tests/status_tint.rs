use bevy::prelude::*;
use lifthrasir_assets::animation::tags::LAYER_BODY;
use lifthrasir_runtime::actors::RenderLayer;
use lifthrasir_runtime::effects::overlays::{
    BodyStateTint, HealthStateTint, Opt3Tint, apply_body_state_tint,
};

#[test]
fn health_tint_composes_with_other_channels_and_clears_independently() {
    let mut app = App::new();
    app.init_resource::<Assets<StandardMaterial>>()
        .add_systems(Update, apply_body_state_tint);
    let material = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial::default());
    let unit = app
        .world_mut()
        .spawn((
            BodyStateTint(Color::srgb(0.5, 0.5, 0.5)),
            Opt3Tint(Color::srgb(1.0, 0.5, 1.0)),
            HealthStateTint(Color::srgb(0.9, 0.4, 0.8)),
        ))
        .id();
    app.world_mut().spawn((
        MeshMaterial3d(material.clone()),
        RenderLayer::body(Handle::default(), LAYER_BODY, vec![]),
        ChildOf(unit),
    ));
    app.update();
    assert_eq!(
        app.world()
            .resource::<Assets<StandardMaterial>>()
            .get(&material)
            .unwrap()
            .base_color,
        Color::srgb(0.45, 0.1, 0.4)
    );
    app.world_mut().entity_mut(unit).remove::<HealthStateTint>();
    app.update();
    assert_eq!(
        app.world()
            .resource::<Assets<StandardMaterial>>()
            .get(&material)
            .unwrap()
            .base_color,
        Color::srgb(0.5, 0.25, 0.5)
    );
}
