pub use lifthrasir_runtime::rendering::billboard::*;

#[cfg(test)]
use bevy::prelude::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::camera::components::CameraFollowTarget;
    use crate::domain::entities::markers::LocalPlayer;
    use moonshine_kind::Instance;

    fn spawn_player(app: &mut App) -> Instance<LocalPlayer> {
        let player = app.world_mut().spawn(LocalPlayer).id();
        Instance::from_entity(app.world().entity(player)).unwrap()
    }

    fn rot_a() -> Quat {
        Quat::from_rotation_y(0.7)
    }

    fn rot_b() -> Quat {
        Quat::from_rotation_y(-1.3)
    }

    fn preview_rot() -> Quat {
        Quat::from_rotation_x(0.4)
    }

    #[test]
    fn world_billboard_faces_primary_camera_with_a_second_camera_present() {
        let mut app = App::new();
        app.add_systems(Update, billboard_rotation_system);

        let player = spawn_player(&mut app);

        // Primary follow camera with rotation A.
        app.world_mut().spawn((
            Camera3d::default(),
            Transform::from_rotation(rot_a()),
            CameraFollowTarget::new(player, Vec3::ZERO),
        ));

        // Secondary camera (e.g. the preview camera) with a different rotation B.
        app.world_mut().spawn((
            Camera3d::default(),
            Transform::from_rotation(rot_b()),
            EquipmentPreviewCamera,
        ));

        let billboard = app
            .world_mut()
            .spawn((Billboard, Transform::default()))
            .id();

        app.update();

        let rotation = app.world().get::<Transform>(billboard).unwrap().rotation;
        assert!(
            rotation.abs_diff_eq(rot_a(), 1e-5),
            "world billboard must face the primary follow camera, not freeze or face the second camera"
        );
    }

    #[test]
    fn world_billboard_faces_menu_preview_camera_without_a_follow_target() {
        // Regression: the character-selection / -creation screens render world
        // billboards with an orthographic camera that has no `CameraFollowTarget`.
        // Requiring a follow target left those sprites unrotated (upside down).
        let mut app = App::new();
        app.add_systems(Update, billboard_rotation_system);

        app.world_mut()
            .spawn((Camera3d::default(), Transform::from_rotation(rot_a())));

        let billboard = app
            .world_mut()
            .spawn((Billboard, Transform::default()))
            .id();

        app.update();

        let rotation = app.world().get::<Transform>(billboard).unwrap().rotation;
        assert!(
            rotation.abs_diff_eq(rot_a(), 1e-5),
            "world billboard must face the menu preview camera even without a follow target"
        );
    }

    #[test]
    fn preview_system_faces_preview_billboards_at_preview_camera() {
        let mut app = App::new();
        app.add_systems(Update, preview_billboard_rotation_system);

        app.world_mut().spawn((
            Camera3d::default(),
            Transform::from_rotation(preview_rot()),
            EquipmentPreviewCamera,
        ));

        let preview_billboard = app
            .world_mut()
            .spawn((Billboard, PreviewBillboard, Transform::default()))
            .id();

        let world_billboard = app
            .world_mut()
            .spawn((Billboard, Transform::default()))
            .id();

        app.update();

        let preview_rotation = app
            .world()
            .get::<Transform>(preview_billboard)
            .unwrap()
            .rotation;
        assert!(
            preview_rotation.abs_diff_eq(preview_rot(), 1e-5),
            "preview billboard must face the preview camera"
        );

        let world_rotation = app
            .world()
            .get::<Transform>(world_billboard)
            .unwrap()
            .rotation;
        assert!(
            world_rotation.abs_diff_eq(Quat::IDENTITY, 1e-5),
            "preview system must not touch world (non-preview) billboards"
        );
    }

    #[test]
    fn preview_system_is_noop_without_preview_camera() {
        let mut app = App::new();
        app.add_systems(Update, preview_billboard_rotation_system);

        let preview_billboard = app
            .world_mut()
            .spawn((Billboard, PreviewBillboard, Transform::default()))
            .id();

        app.update();

        let rotation = app
            .world()
            .get::<Transform>(preview_billboard)
            .unwrap()
            .rotation;
        assert!(
            rotation.abs_diff_eq(Quat::IDENTITY, 1e-5),
            "preview system must be a no-op when no preview camera exists"
        );
    }

    #[test]
    fn world_system_skips_preview_billboards() {
        let mut app = App::new();
        app.add_systems(Update, billboard_rotation_system);

        let player = spawn_player(&mut app);
        app.world_mut().spawn((
            Camera3d::default(),
            Transform::from_rotation(rot_a()),
            CameraFollowTarget::new(player, Vec3::ZERO),
        ));

        let preview_billboard = app
            .world_mut()
            .spawn((Billboard, PreviewBillboard, Transform::default()))
            .id();

        app.update();

        let rotation = app
            .world()
            .get::<Transform>(preview_billboard)
            .unwrap()
            .rotation;
        assert!(
            rotation.abs_diff_eq(Quat::IDENTITY, 1e-5),
            "world system must skip preview-layer billboards"
        );
    }
}
