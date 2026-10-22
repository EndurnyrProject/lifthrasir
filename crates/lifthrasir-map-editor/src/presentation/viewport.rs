//! Editor camera setup, map activation framing and ambient lighting.
use bevy::{
    camera::Exposure,
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    prelude::*,
};

use crate::{
    application::EditorSystems,
    infrastructure::content::{LifMapData, MapSceneRequest},
};

#[derive(Component)]
pub(super) struct EditorCamera;

#[derive(Resource)]
pub(crate) struct MapAssetPath(pub Option<String>);

/// Shared grid convention for the viewport and collision tools.
pub(super) const CELL_SIZE: f32 = 5.0;

pub(super) struct ViewportPlugin;

impl Plugin for ViewportPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FreeCameraPlugin)
            .add_systems(Startup, setup)
            .add_systems(Update, apply_map_lighting.in_set(EditorSystems::Present));
    }
}

fn setup(mut commands: Commands, map_path: Res<MapAssetPath>) {
    commands.spawn((
        Camera3d::default(),
        EditorCamera,
        camera_settings(),
        Transform::from_xyz(600.0, 500.0, 700.0).looking_at(Vec3::new(600.0, 0.0, -600.0), Vec3::Y),
    ));
    if let Some(path) = &map_path.0 {
        commands.spawn(MapSceneRequest {
            path: format!("ro://{path}"),
        });
    }
}

fn camera_settings() -> FreeCamera {
    // Bevy's default 5 units/s is intended for a small example scene.
    FreeCamera {
        walk_speed: 300.0,
        run_speed: 900.0,
        key_up: KeyCode::Space,
        key_down: KeyCode::AltLeft,
        ..default()
    }
}

/// The host owns map activation policy; the shared renderer supplies metadata.
fn apply_map_lighting(
    mut commands: Commands,
    maps: Query<&LifMapData, Added<LifMapData>>,
    mut cameras: Query<(&mut Transform, &mut Exposure), With<EditorCamera>>,
) {
    for data in &maps {
        let width = data.altitude.width as f32 * CELL_SIZE;
        let height = data.altitude.height as f32 * CELL_SIZE;
        let center = Vec3::new(width * 0.5, 0.0, -height * 0.5);
        let distance = width.max(height).max(50.0);
        for (mut transform, mut exposure) in &mut cameras {
            *transform = Transform::from_translation(center + Vec3::new(0.0, distance, distance))
                .looking_at(center, Vec3::Y);
            exposure.ev100 = data.meta.exposure_ev100;
        }
        commands.insert_resource(GlobalAmbientLight {
            color: Color::srgb(
                data.meta.ambient_color[0],
                data.meta.ambient_color[1],
                data.meta.ambient_color[2],
            ),
            brightness: data.meta.ambient_brightness,
            affects_lightmapped_meshes: false,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_uses_editor_conventions() {
        let camera = camera_settings();
        assert_eq!(camera.key_up, KeyCode::Space);
        assert_eq!(camera.key_down, KeyCode::AltLeft);
        assert_eq!(camera.walk_speed, 300.0);
    }
}
