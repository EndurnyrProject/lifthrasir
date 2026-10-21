use bevy::{
    camera::Exposure,
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    dev_tools::infinite_grid::{InfiniteGrid, InfiniteGridPlugin, InfiniteGridSettings},
    prelude::*,
};

use crate::{
    application::EditorSystems,
    infrastructure::content::{LifMapData, MapSceneRequest},
};

#[derive(Component)]
pub(super) struct EditorCamera;

#[derive(Resource)]
pub(crate) struct MapAssetPath(pub String);

/// Shared grid convention for the viewport and collision tools.
pub(super) const CELL_SIZE: f32 = 5.0;

pub(super) struct ViewportPlugin;

impl Plugin for ViewportPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((FreeCameraPlugin, InfiniteGridPlugin))
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (apply_map_lighting, toggle_infinite_grid).in_set(EditorSystems::Present),
            );
    }
}

fn setup(mut commands: Commands, map_path: Res<MapAssetPath>) {
    commands.spawn((
        Camera3d::default(),
        EditorCamera,
        camera_settings(),
        Transform::from_xyz(600.0, 500.0, 700.0).looking_at(Vec3::new(600.0, 0.0, -600.0), Vec3::Y),
    ));
    commands.spawn(MapSceneRequest {
        path: format!("ro://{}", map_path.0),
    });
    commands.spawn((InfiniteGrid, grid_settings()));
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

fn grid_settings() -> InfiniteGridSettings {
    InfiniteGridSettings {
        scale: CELL_SIZE.recip(),
        fadeout_distance: 1_500.0,
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

fn toggle_infinite_grid(
    keys: Res<ButtonInput<KeyCode>>,
    mut grids: Query<&mut Visibility, With<InfiniteGrid>>,
) {
    if !keys.just_pressed(KeyCode::KeyP) {
        return;
    }
    for mut visibility in &mut grids {
        *visibility = match *visibility {
            Visibility::Hidden => Visibility::Visible,
            Visibility::Inherited | Visibility::Visible => Visibility::Hidden,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_and_camera_use_editor_conventions() {
        assert_eq!(grid_settings().scale.recip(), CELL_SIZE);
        let camera = camera_settings();
        assert_eq!(camera.key_up, KeyCode::Space);
        assert_eq!(camera.key_down, KeyCode::AltLeft);
        assert_eq!(camera.walk_speed, 300.0);
    }

    #[test]
    fn grid_toggle_is_an_in_memory_viewport_action() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, toggle_infinite_grid);
        let grid = app.world_mut().spawn(InfiniteGrid).id();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyP);
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(grid),
            Some(&Visibility::Hidden)
        );
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(grid),
            Some(&Visibility::Visible)
        );
    }
}
