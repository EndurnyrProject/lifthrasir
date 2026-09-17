use bevy::ecs::entity::EntityHashMap;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::input::touch::TouchPhase;
use bevy::picking::{backend::HitData, hover::HoverMap, pointer::PointerId};
use bevy::prelude::*;
use game_engine::domain::camera::{
    components::{CameraFollowSettings, CameraFollowTarget},
    resources::{ActiveCameraProfile, CameraRotationDelta},
    systems::camera_follow_system,
};
use game_engine::domain::input::UiFocus;
use moonshine_kind::Instance;

fn test_app() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<UiFocus>()
        .init_resource::<HoverMap>()
        .init_resource::<CameraRotationDelta>()
        .init_resource::<ActiveCameraProfile>()
        .add_message::<MouseWheel>()
        .add_systems(Update, camera_follow_system);
    let camera = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            CameraFollowTarget::new(Instance::PLACEHOLDER, Vec3::ZERO),
            CameraFollowSettings::default(),
        ))
        .id();
    (app, camera)
}

fn hover(app: &mut App, entity: Entity) {
    let mut hits = EntityHashMap::default();
    hits.insert(entity, HitData::new(Entity::PLACEHOLDER, 0.0, None, None));
    app.world_mut()
        .resource_mut::<HoverMap>()
        .insert(PointerId::Mouse, hits);
}

fn scroll(app: &mut App, unit: MouseScrollUnit, y: f32) {
    app.world_mut().write_message(MouseWheel {
        unit,
        x: 0.0,
        y,
        window: Entity::PLACEHOLDER,
        phase: TouchPhase::Moved,
    });
    app.update();
}

#[test]
fn wheel_over_scrollbar_thumb_does_not_zoom() {
    let (mut app, camera) = test_app();
    let thumb = app
        .world_mut()
        .spawn(bevy::ui_widgets::ScrollbarThumb::default())
        .id();
    hover(&mut app, thumb);
    let original = app
        .world()
        .get::<CameraFollowSettings>(camera)
        .unwrap()
        .offset;

    scroll(&mut app, MouseScrollUnit::Pixel, -50.0);
    assert_eq!(
        app.world()
            .get::<CameraFollowSettings>(camera)
            .unwrap()
            .offset,
        original
    );
}

#[test]
fn wheel_zooms_over_world_and_through_nonblocking_ui() {
    let (mut app, camera) = test_app();
    let window = app.world_mut().spawn(Window::default()).id();
    let actor = app.world_mut().spawn(Pickable::default()).id();
    let overlay = app
        .world_mut()
        .spawn((
            Node::default(),
            Pickable {
                should_block_lower: false,
                ..default()
            },
        ))
        .id();

    for entity in [window, actor, overlay] {
        app.world_mut()
            .entity_mut(camera)
            .insert(CameraFollowSettings::default());
        hover(&mut app, entity);
        scroll(&mut app, MouseScrollUnit::Line, 1.0);
        let distance = app
            .world()
            .get::<CameraFollowSettings>(camera)
            .unwrap()
            .offset
            .length();
        assert!(
            (distance - 187.13203).abs() < 0.001,
            "zoom blocked by {entity:?}"
        );
    }
}

#[test]
fn wheel_over_ui_does_not_zoom_or_replay_after_leaving() {
    let (mut app, camera) = test_app();
    let panel = app.world_mut().spawn(Node::default()).id();
    hover(&mut app, panel);
    let original = app
        .world()
        .get::<CameraFollowSettings>(camera)
        .unwrap()
        .offset;

    scroll(&mut app, MouseScrollUnit::Line, -1.0);
    assert_eq!(
        app.world()
            .get::<CameraFollowSettings>(camera)
            .unwrap()
            .offset,
        original
    );

    app.world_mut().resource_mut::<HoverMap>().clear();
    app.update();
    assert_eq!(
        app.world()
            .get::<CameraFollowSettings>(camera)
            .unwrap()
            .offset,
        original
    );
}
