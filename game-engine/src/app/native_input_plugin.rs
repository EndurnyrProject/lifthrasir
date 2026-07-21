use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::picking::hover::HoverMap;
use bevy::picking::pointer::PointerId;
use bevy::prelude::*;
use bevy::window::CursorMoved;
use bevy_auto_plugin::prelude::{AutoPlugin, auto_add_system};

use crate::domain::camera::CameraRotationDelta;
use crate::domain::input::{ForwardedCursorPosition, ForwardedMouseClick, ui_unfocused};
use crate::domain::system_sets::InputSystems;

/// Feeds engine input resources from native window input.
#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct NativeInputPlugin;

#[auto_add_system(
    plugin = NativeInputPlugin,
    schedule = Update,
    config(before = InputSystems::Raycast, run_if = ui_unfocused)
)]
fn forward_cursor_position(
    mut moved: MessageReader<CursorMoved>,
    mut cursor: ResMut<ForwardedCursorPosition>,
) {
    for event in moved.read() {
        cursor.position = Some(event.position);
    }
}

#[auto_add_system(
    plugin = NativeInputPlugin,
    schedule = Update,
    config(before = InputSystems::Raycast, run_if = ui_unfocused)
)]
fn forward_mouse_click(
    buttons: Res<ButtonInput<MouseButton>>,
    cursor: Res<ForwardedCursorPosition>,
    hover_map: Res<HoverMap>,
    windows: Query<(), With<Window>>,
    mut click: ResMut<ForwardedMouseClick>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    if pointer_over_pickable(&hover_map, &windows) {
        return;
    }
    click.position = cursor.position;
}

/// Whether the mouse pointer is over a pickable entity that should swallow the
/// raw world click, so it only fires on empty ground. Picked sprite bodies (mesh
/// picking) and windows that opt into picking enter the hover map; `Pickable::IGNORE`
/// elements (the always-on HUD) and the terrain (no `Pickable`) never do.
///
/// The pointer's own `Window` entity is always present in the hover map, so it is
/// excluded here — otherwise every world click would be suppressed.
fn pointer_over_pickable(hover_map: &HoverMap, windows: &Query<(), With<Window>>) -> bool {
    hover_map
        .get(&PointerId::Mouse)
        .is_some_and(|hits| hits.keys().any(|entity| !windows.contains(*entity)))
}

#[auto_add_system(
    plugin = NativeInputPlugin,
    schedule = Update,
    config(before = InputSystems::Raycast, run_if = ui_unfocused)
)]
fn forward_camera_rotation(
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    mut rotation: ResMut<CameraRotationDelta>,
) {
    if !buttons.pressed(MouseButton::Right) {
        return;
    }
    rotation.delta_x += motion.delta.x;
    rotation.delta_y += motion.delta.y;
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::domain::input::UiFocus;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<ForwardedCursorPosition>();
        app.init_resource::<ForwardedMouseClick>();
        app.init_resource::<CameraRotationDelta>();
        app.init_resource::<ButtonInput<MouseButton>>();
        app.init_resource::<AccumulatedMouseMotion>();
        app.init_resource::<UiFocus>();
        app.init_resource::<HoverMap>();
        app.add_message::<CursorMoved>();
        app.add_systems(
            Update,
            (
                forward_cursor_position,
                forward_mouse_click,
                forward_camera_rotation,
            )
                .run_if(ui_unfocused),
        );
        app
    }

    #[test]
    fn cursor_moved_updates_forwarded_position() {
        let mut app = test_app();
        let window = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(CursorMoved {
            window,
            position: Vec2::new(120.0, 240.0),
            delta: None,
        });
        app.update();

        let cursor = app.world().resource::<ForwardedCursorPosition>();
        assert_eq!(cursor.position, Some(Vec2::new(120.0, 240.0)));
    }

    #[test]
    fn left_click_sets_forwarded_click() {
        let mut app = test_app();
        app.world_mut()
            .resource_mut::<ForwardedCursorPosition>()
            .position = Some(Vec2::new(33.0, 44.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        let click = app.world().resource::<ForwardedMouseClick>();
        assert_eq!(click.position, Some(Vec2::new(33.0, 44.0)));
    }

    #[test]
    fn right_drag_accumulates_camera_rotation() {
        let mut app = test_app();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.world_mut()
            .resource_mut::<AccumulatedMouseMotion>()
            .delta = Vec2::new(5.0, -3.0);
        app.update();

        let delta = app.world().resource::<CameraRotationDelta>();
        assert_eq!(delta.delta_x, 5.0);
        assert_eq!(delta.delta_y, -3.0);
    }

    #[test]
    fn click_not_forwarded_while_ui_focused() {
        let mut app = test_app();
        app.world_mut().resource_mut::<UiFocus>().text_input_active = true;
        app.world_mut()
            .resource_mut::<ForwardedCursorPosition>()
            .position = Some(Vec2::new(33.0, 44.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        let click = app.world().resource::<ForwardedMouseClick>();
        assert_eq!(click.position, None);
    }

    #[test]
    fn click_not_forwarded_while_pointer_over_ui() {
        use bevy::ecs::entity::EntityHashMap;
        use bevy::picking::backend::HitData;

        let mut app = test_app();
        let ui_node = app.world_mut().spawn(Node::default()).id();
        let mut hits = EntityHashMap::default();
        hits.insert(ui_node, HitData::new(Entity::PLACEHOLDER, 0.0, None, None));
        app.world_mut()
            .resource_mut::<HoverMap>()
            .insert(PointerId::Mouse, hits);
        app.world_mut()
            .resource_mut::<ForwardedCursorPosition>()
            .position = Some(Vec2::new(33.0, 44.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        let click = app.world().resource::<ForwardedMouseClick>();
        assert_eq!(click.position, None);
    }

    #[test]
    fn click_forwarded_when_only_hit_is_the_window() {
        use bevy::ecs::entity::EntityHashMap;
        use bevy::picking::backend::HitData;

        // The pointer's own `Window` entity is always in the hover map. It must not
        // count as a pickable target, or every world (terrain) click is swallowed.
        let mut app = test_app();
        let window = app.world_mut().spawn(Window::default()).id();
        let mut hits = EntityHashMap::default();
        hits.insert(window, HitData::new(Entity::PLACEHOLDER, 0.0, None, None));
        app.world_mut()
            .resource_mut::<HoverMap>()
            .insert(PointerId::Mouse, hits);
        app.world_mut()
            .resource_mut::<ForwardedCursorPosition>()
            .position = Some(Vec2::new(33.0, 44.0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        let click = app.world().resource::<ForwardedMouseClick>();
        assert_eq!(click.position, Some(Vec2::new(33.0, 44.0)));
    }
}
