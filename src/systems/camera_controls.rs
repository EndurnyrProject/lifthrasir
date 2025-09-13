use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;

#[derive(Component)]
pub struct CameraController {
    pub move_speed: f32,
    pub zoom_speed: f32,
    pub mouse_sensitivity: f32,
    pub is_dragging: bool,
}

impl Default for CameraController {
    fn default() -> Self {
        Self {
            move_speed: 500.0,
            zoom_speed: 200.0,
            mouse_sensitivity: 0.5,
            is_dragging: false,
        }
    }
}

pub fn camera_movement_system(
    time: Res<Time>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    mut mouse_motion_events: EventReader<MouseMotion>,
    mut mouse_wheel_events: EventReader<MouseWheel>,
    mut camera_query: Query<(&mut Transform, &mut CameraController), With<Camera3d>>,
) {
    for (mut transform, mut controller) in camera_query.iter_mut() {
        let delta = time.delta_secs();

        // Handle mouse dragging for camera movement
        if mouse_button_input.pressed(MouseButton::Left) {
            controller.is_dragging = true;

            for mouse_motion in mouse_motion_events.read() {
                let pan_speed = controller.move_speed * controller.mouse_sensitivity;

                // Get camera's right and up vectors
                let right = transform.rotation * Vec3::X;
                let up = transform.rotation * Vec3::Y;

                // Pan camera based on mouse movement
                transform.translation -= right * mouse_motion.delta.x * pan_speed * delta;
                transform.translation += up * mouse_motion.delta.y * pan_speed * delta;
            }
        } else {
            controller.is_dragging = false;
        }

        // Handle mouse wheel for zooming
        for mouse_wheel in mouse_wheel_events.read() {
            let zoom_delta = mouse_wheel.y * controller.zoom_speed * delta;

            // Move camera forward/backward along its look direction
            let forward = transform.rotation * Vec3::NEG_Z;
            transform.translation += forward * zoom_delta;
        }

        // Handle keyboard movement
        let mut movement = Vec3::ZERO;

        if keyboard_input.pressed(KeyCode::KeyW) || keyboard_input.pressed(KeyCode::ArrowUp) {
            movement.z -= 1.0;
        }
        if keyboard_input.pressed(KeyCode::KeyS) || keyboard_input.pressed(KeyCode::ArrowDown) {
            movement.z += 1.0;
        }
        if keyboard_input.pressed(KeyCode::KeyA) || keyboard_input.pressed(KeyCode::ArrowLeft) {
            movement.x -= 1.0;
        }
        if keyboard_input.pressed(KeyCode::KeyD) || keyboard_input.pressed(KeyCode::ArrowRight) {
            movement.x += 1.0;
        }
        if keyboard_input.pressed(KeyCode::KeyQ) {
            movement.y -= 1.0;
        }
        if keyboard_input.pressed(KeyCode::KeyE) {
            movement.y += 1.0;
        }

        if movement != Vec3::ZERO {
            movement = movement.normalize();

            // Transform movement to camera space
            let right = transform.rotation * Vec3::X;
            let up = Vec3::Y; // Keep up as world up
            let forward = transform.rotation * Vec3::NEG_Z;

            let movement_world = right * movement.x + up * movement.y + forward * movement.z;
            transform.translation += movement_world * controller.move_speed * delta;
        }

        // Handle camera rotation with right mouse button
        if mouse_button_input.pressed(MouseButton::Right) {
            for mouse_motion in mouse_motion_events.read() {
                let sensitivity = controller.mouse_sensitivity * 0.001;

                // Store translation to avoid borrow checker issues
                let translation = transform.translation;

                // Yaw (rotate around Y axis)
                let yaw_delta = -mouse_motion.delta.x * sensitivity;
                transform.rotate_around(translation, Quat::from_rotation_y(yaw_delta));

                // Pitch (rotate around local X axis)
                let pitch_delta = -mouse_motion.delta.y * sensitivity;
                let right = transform.rotation * Vec3::X;
                transform.rotate_around(translation, Quat::from_axis_angle(right, pitch_delta));
            }
        }

        // Reset camera position with R key
        if keyboard_input.just_pressed(KeyCode::KeyR) {
            transform.translation = Vec3::new(0.0, 1500.0, 1000.0);
            transform.look_at(Vec3::ZERO, Vec3::Y);
        }
    }
}
