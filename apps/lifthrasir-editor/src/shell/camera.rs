//! The editor camera: free fly, or an RO-like orbit at a fixed pitch.

use bevy::{
    camera::Exposure,
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit},
    prelude::*,
    window::PrimaryWindow,
};
use bevy_egui::PrimaryEguiContext;
use bevy_kira_audio::SpatialAudioReceiver;
use lifthrasir_runtime::audio::ContentAudioListener;
use std::f32::consts::FRAC_PI_4;

/// How the camera reacts to input. `C` toggles (egui uses Tab for widget focus).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CameraMode {
    /// WASD, Q/E down/up, Shift faster, right-mouse look.
    Fly,
    /// Fixed pitch, scroll zoom, right-mouse drag yaws around `focus`.
    #[default]
    Orbit,
}

/// The single editor camera.
#[derive(Component, Debug, Clone, Copy)]
pub struct EditorCamera {
    pub mode: CameraMode,
    /// The ground point the orbit camera yaws around.
    pub focus: Vec3,
}

/// Orbit pitch, looking down at 45 degrees like the RO camera.
pub const ORBIT_PITCH: f32 = -FRAC_PI_4;
const FLY_SPEED: f32 = 150.0;
const FLY_FAST_FACTOR: f32 = 4.0;
const LOOK_SENSITIVITY: f32 = 0.003;
const ZOOM_STEP: f32 = 0.1;
const SCROLL_PIXELS_PER_LINE: f32 = 50.0;
const MIN_DISTANCE: f32 = 10.0;
const MAX_DISTANCE: f32 = 5000.0;
const START_DISTANCE: f32 = 200.0;

pub fn spawn_editor_camera(mut commands: Commands) {
    let focus = Vec3::ZERO;
    commands.spawn((
        Camera3d::default(),
        EditorCamera {
            mode: CameraMode::Orbit,
            focus,
        },
        orbit_transform(focus, 0.0, START_DISTANCE),
        Exposure::default(),
        Projection::Perspective(PerspectiveProjection {
            far: 10000.0,
            ..default()
        }),
        ContentAudioListener,
        SpatialAudioReceiver,
        // Explicit, since the preview camera also renders and egui would
        // otherwise attach to whichever camera it sees first.
        PrimaryEguiContext,
    ));
}

/// World-space displacement of one fly step. `input` is in camera-local axes
/// (x right, y up, -z forward); its length is ignored.
pub fn fly_step(rotation: Quat, input: Vec3, distance: f32) -> Vec3 {
    rotation * input.normalize_or_zero() * distance
}

/// Orbit camera at `distance` from `focus`, yawed by `yaw` around world Y and
/// pitched by [`ORBIT_PITCH`], looking at `focus`.
pub fn orbit_transform(focus: Vec3, yaw: f32, distance: f32) -> Transform {
    let rotation = Quat::from_euler(EulerRot::YXZ, yaw, ORBIT_PITCH, 0.0);
    Transform::from_translation(focus + rotation * Vec3::Z * distance).with_rotation(rotation)
}

/// Points `camera` at the centre of the `min..max` box from above, at the
/// orbit pitch, far enough to see all of it. Callers also set
/// `EditorCamera.focus` to `(min + max) / 2` so orbit zoom and yaw pivot there.
pub fn frame_bounds(camera: &mut Transform, min: Vec3, max: Vec3) {
    let centre = (min + max) * 0.5;
    let extent = (max - min).max_element().max(MIN_DISTANCE);
    *camera = orbit_transform(centre, 0.0, extent * std::f32::consts::SQRT_2);
}

fn fly_look(rotation: Quat, delta: Vec2) -> Quat {
    let (yaw, pitch, _) = rotation.to_euler(EulerRot::YXZ);
    let yaw = yaw - delta.x * LOOK_SENSITIVITY;
    let pitch = (pitch - delta.y * LOOK_SENSITIVITY).clamp(-1.54, 1.54);
    Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0)
}

/// Where `ray` meets the `y = 0` ground plane.
// NOTE: a flat plane until terrain picking exists (Task 12's TerrainCursor).
fn ground_hit(ray: Ray3d) -> Option<Vec3> {
    ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y))
        .map(|distance| ray.get_point(distance))
}

pub fn toggle_camera_mode(
    keys: Res<ButtonInput<KeyCode>>,
    mut cameras: Query<(&mut EditorCamera, &mut Transform)>,
) {
    if !keys.just_pressed(KeyCode::KeyC) {
        return;
    }
    for (mut camera, mut transform) in &mut cameras {
        camera.mode = match camera.mode {
            CameraMode::Fly => CameraMode::Orbit,
            CameraMode::Orbit => CameraMode::Fly,
        };
        if camera.mode == CameraMode::Fly {
            continue;
        }
        let forward = transform.forward();
        camera.focus = ground_hit(Ray3d::new(transform.translation, forward))
            .unwrap_or(transform.translation + forward * START_DISTANCE);
        let distance = transform
            .translation
            .distance(camera.focus)
            .clamp(MIN_DISTANCE, MAX_DISTANCE);
        let (yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
        *transform = orbit_transform(camera.focus, yaw, distance);
    }
}

pub fn fly_move(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut cameras: Query<(&EditorCamera, &mut Transform)>,
) {
    let axis = |positive: KeyCode, negative: KeyCode| {
        keys.pressed(positive) as i8 as f32 - keys.pressed(negative) as i8 as f32
    };
    let input = Vec3::new(
        axis(KeyCode::KeyD, KeyCode::KeyA),
        axis(KeyCode::KeyE, KeyCode::KeyQ),
        axis(KeyCode::KeyS, KeyCode::KeyW),
    );
    if input == Vec3::ZERO {
        return;
    }
    let fast = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let speed = FLY_SPEED * if fast { FLY_FAST_FACTOR } else { 1.0 };
    for (camera, mut transform) in &mut cameras {
        if camera.mode != CameraMode::Fly {
            continue;
        }
        let step = fly_step(transform.rotation, input, speed * time.delta_secs());
        transform.translation += step;
    }
}

pub fn fly_look_input(
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    mut cameras: Query<(&EditorCamera, &mut Transform)>,
) {
    if !mouse.pressed(MouseButton::Right) || motion.delta == Vec2::ZERO {
        return;
    }
    for (camera, mut transform) in &mut cameras {
        if camera.mode == CameraMode::Fly {
            transform.rotation = fly_look(transform.rotation, motion.delta);
        }
    }
}

pub fn orbit_input(
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<(&mut EditorCamera, &mut Transform, &Camera, &GlobalTransform)>,
) {
    let cursor = windows.single().ok().and_then(Window::cursor_position);
    let lines = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / SCROLL_PIXELS_PER_LINE,
    };
    for (mut editor, mut transform, camera, global) in &mut cameras {
        if editor.mode != CameraMode::Orbit {
            continue;
        }
        if mouse.just_pressed(MouseButton::Right)
            && let Some(hit) = cursor
                .and_then(|cursor| camera.viewport_to_world(global, cursor).ok())
                .and_then(ground_hit)
        {
            editor.focus = hit;
        }
        if mouse.pressed(MouseButton::Right) && motion.delta.x != 0.0 {
            let yaw = Quat::from_rotation_y(-motion.delta.x * LOOK_SENSITIVITY);
            transform.rotate_around(editor.focus, yaw);
        }
        if lines != 0.0 {
            let factor = (1.0 - lines * ZOOM_STEP).clamp(0.5, 1.5);
            let offset = (transform.translation - editor.focus) * factor;
            transform.translation = editor.focus + offset.clamp_length(MIN_DISTANCE, MAX_DISTANCE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        a.abs_diff_eq(b, 1e-4)
    }

    #[test]
    fn a_fly_step_follows_the_camera_basis() {
        // Yawed -90 degrees: the camera faces +X, so its right is +Z.
        let rotation = Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2);

        assert!(close(
            fly_step(rotation, Vec3::NEG_Z, 10.0),
            Vec3::new(10.0, 0.0, 0.0)
        ));
        assert!(close(
            fly_step(rotation, Vec3::X, 10.0),
            Vec3::new(0.0, 0.0, 10.0)
        ));
        assert!((fly_step(rotation, Vec3::new(1.0, 0.0, -1.0), 2.0).length() - 2.0).abs() < 1e-5);
    }

    #[test]
    fn the_orbit_keeps_its_pitch_and_distance_after_a_yaw() {
        let focus = Vec3::new(30.0, 0.0, -40.0);

        let transform = orbit_transform(focus, 1.2, 250.0);

        let (yaw, pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
        assert!((pitch - ORBIT_PITCH).abs() < 1e-5);
        assert!((yaw - 1.2).abs() < 1e-5);
        assert!((transform.translation.distance(focus) - 250.0).abs() < 1e-3);
        let to_focus = (focus - transform.translation).normalize();
        assert!(close(*transform.forward(), to_focus));
    }

    #[test]
    fn framing_looks_at_the_centre_of_the_bounds() {
        let mut camera = Transform::default();
        let (min, max) = (Vec3::new(0.0, -5.0, -400.0), Vec3::new(600.0, 5.0, 0.0));

        frame_bounds(&mut camera, min, max);

        let to_centre = (Vec3::new(300.0, 0.0, -200.0) - camera.translation).normalize();
        assert!(close(*camera.forward(), to_centre));
        let (_, pitch, _) = camera.rotation.to_euler(EulerRot::YXZ);
        assert!((pitch - ORBIT_PITCH).abs() < 1e-5);
    }
}
