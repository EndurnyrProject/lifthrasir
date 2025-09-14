use bevy::prelude::*;

#[derive(Component)]
pub struct CameraController {
    pub enabled: bool,
    pub key_forward: KeyCode,
    pub key_backward: KeyCode,
    pub key_left: KeyCode,
    pub key_right: KeyCode,
    pub key_up: KeyCode,
    pub key_down: KeyCode,
    pub key_run: KeyCode,
    pub mouse_key_cursor_grab: MouseButton,
    pub mouse_key_rotate: MouseButton,
    pub keyboard_key_toggle_cursor_grab: KeyCode,
    pub walk_speed: f32,
    pub run_speed: f32,
    pub friction: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub velocity: Vec3,
    pub key_reset: KeyCode,
    pub zoom_speed: f32,
    pub pan_speed: f32,
    pub rotate_speed: f32,
    pub initial_position: Vec3,
    pub initial_target: Vec3,
}

impl Default for CameraController {
    fn default() -> Self {
        Self {
            enabled: true,
            key_forward: KeyCode::KeyW,
            key_backward: KeyCode::KeyS,
            key_left: KeyCode::KeyA,
            key_right: KeyCode::KeyD,
            key_up: KeyCode::KeyE,
            key_down: KeyCode::KeyQ,
            key_run: KeyCode::ShiftLeft,
            mouse_key_cursor_grab: MouseButton::Left,
            mouse_key_rotate: MouseButton::Right,
            keyboard_key_toggle_cursor_grab: KeyCode::KeyM,
            walk_speed: 50.0,
            run_speed: 250.0,
            friction: 0.5,
            pitch: 0.0,
            yaw: 0.0,
            velocity: Vec3::ZERO,
            key_reset: KeyCode::KeyR,
            zoom_speed: 100.0,
            pan_speed: 2.0,
            rotate_speed: 0.003,
            initial_position: Vec3::ZERO,
            initial_target: Vec3::ZERO,
        }
    }
}
