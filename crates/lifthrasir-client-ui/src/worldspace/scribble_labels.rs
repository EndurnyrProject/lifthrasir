//! Cell-anchored Scribble messages, independent of the caster's visibility.

use bevy::prelude::*;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::skill_units::components::ScribbleMessage;

use super::{WorldCameraFilter, WorldspaceFont, viewport_to_ui};

const LABEL_WIDTH: f32 = 260.0;
const LABEL_GAP: f32 = 18.0;

pub struct ScribbleLabelPlugin;

impl Plugin for ScribbleLabelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (sync_scribble_labels, follow_scribble_labels)
                .chain()
                .after(bevy::transform::TransformSystems::Propagate)
                .after(bevy::camera::CameraUpdateSystems)
                .before(bevy::ui::UiSystems::Prepare)
                .run_if(in_state(GameState::InGame)),
        );
    }
}

#[derive(Component)]
struct ScribbleLabel {
    target: Entity,
}

fn sync_scribble_labels(
    mut commands: Commands,
    cells: Query<(Entity, &ScribbleMessage)>,
    labels: Query<(Entity, &ScribbleLabel)>,
    font: Res<WorldspaceFont>,
) {
    for (target, message) in &cells {
        if labels.iter().any(|(_, label)| label.target == target) {
            continue;
        }
        let text_font = TextFont {
            font: font.0.clone().into(),
            font_size: 13.0.into(),
            ..default()
        };
        commands
            .spawn_scene(bsn! {
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(LABEL_WIDTH),
                }
                Text({message.0.clone()})
                TextColor(lifthrasir_ui::theme::TEXT)
                TextLayout::justify(Justify::Center)
                TextShadow::default()
                GlobalZIndex({super::WORLD_OVERLAY_Z + 100})
                Visibility::Hidden
                Pickable::IGNORE
            })
            .insert((
                text_font,
                ScribbleLabel { target },
                DespawnOnExit(GameState::InGame),
            ));
    }
    for (entity, label) in &labels {
        if cells.get(label.target).is_err() {
            commands.entity(entity).despawn();
        }
    }
}

fn follow_scribble_labels(
    camera: Query<(&Camera, &GlobalTransform), WorldCameraFilter>,
    cells: Query<&GlobalTransform, With<ScribbleMessage>>,
    ui_scale: Res<UiScale>,
    mut labels: Query<(&ScribbleLabel, &mut Node, &mut Visibility)>,
) {
    let camera = camera.single().ok();
    for (label, mut node, mut visibility) in &mut labels {
        let projected = camera.and_then(|(camera, camera_transform)| {
            let target = cells.get(label.target).ok()?;
            camera
                .world_to_viewport(camera_transform, target.translation())
                .ok()
        });
        let Some(screen) = projected else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let position = viewport_to_ui(screen, &ui_scale);
        node.left = Val::Px(position.x - LABEL_WIDTH / 2.0);
        node.top = Val::Px(position.y - LABEL_GAP);
        *visibility = Visibility::Visible;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            bevy::scene::ScenePlugin,
            StatesPlugin,
            ScribbleLabelPlugin,
        ))
        .init_state::<GameState>()
        .insert_resource(WorldspaceFont(Handle::default()))
        .init_resource::<UiScale>();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();
        app
    }

    #[test]
    fn scribble_labels_follow_cell_lifetime_and_replacement_without_a_caster_or_camera() {
        let mut app = app();
        let group = app.world_mut().spawn_empty().id();
        let cell = app
            .world_mut()
            .spawn((
                ScribbleMessage("Hello".into()),
                GlobalTransform::default(),
                ChildOf(group),
            ))
            .id();
        app.update();
        app.update();
        let world = app.world_mut();
        let (old_label, label, text, visibility, pickable) = world
            .query::<(Entity, &ScribbleLabel, &Text, &Visibility, &Pickable)>()
            .single(world)
            .unwrap();
        assert_eq!(label.target, cell);
        assert_eq!(text.0, "Hello");
        assert_eq!(*visibility, Visibility::Hidden);
        assert!(!pickable.should_block_lower);
        assert!(!pickable.is_hoverable);

        app.world_mut().entity_mut(cell).despawn();
        let replacement = app
            .world_mut()
            .spawn((
                ScribbleMessage("Replacement".into()),
                GlobalTransform::default(),
                ChildOf(group),
            ))
            .id();
        app.update();
        let world = app.world_mut();
        assert!(world.get_entity(old_label).is_err());
        let (label, text) = world
            .query::<(&ScribbleLabel, &Text)>()
            .single(world)
            .unwrap();
        assert_eq!(label.target, replacement);
        assert_eq!(text.0, "Replacement");

        app.world_mut().entity_mut(group).despawn();
        app.update();
        let world = app.world_mut();
        assert_eq!(world.query::<&ScribbleLabel>().iter(world).count(), 0);
    }

    #[test]
    fn scribble_projects_the_cell_with_ui_scale_and_hides_without_the_world_camera() {
        use bevy::camera::{ComputedCameraValues, RenderTargetInfo};
        use lifthrasir_runtime::rendering::billboard::EquipmentPreviewCamera;

        let mut app = app();
        app.world_mut().resource_mut::<UiScale>().0 = 2.0;
        let camera = Camera {
            computed: ComputedCameraValues {
                clip_from_view: Mat4::IDENTITY,
                target_info: Some(RenderTargetInfo {
                    physical_size: UVec2::new(800, 600),
                    scale_factor: 1.0,
                }),
                ..default()
            },
            ..default()
        };
        let world_camera = app
            .world_mut()
            .spawn((camera.clone(), Camera3d::default()))
            .id();
        app.world_mut()
            .spawn((camera, Camera3d::default(), EquipmentPreviewCamera));
        let cell = app
            .world_mut()
            .spawn((
                ScribbleMessage("Hello".into()),
                GlobalTransform::from_translation(Vec3::new(0.5, 0.25, 0.5)),
            ))
            .id();
        app.update();
        let world = app.world_mut();
        let (label_entity, node, visibility) = world
            .query_filtered::<(Entity, &Node, &Visibility), With<ScribbleLabel>>()
            .single(world)
            .unwrap();
        assert_eq!(node.left, Val::Px(170.0));
        assert_eq!(node.top, Val::Px(94.5));
        assert_eq!(*visibility, Visibility::Visible);

        app.world_mut()
            .entity_mut(cell)
            .insert(GlobalTransform::from_translation(Vec3::new(0.0, 0.0, 0.5)));
        app.update();
        let node = app.world().get::<Node>(label_entity).unwrap();
        assert_eq!(node.left, Val::Px(70.0));
        assert_eq!(node.top, Val::Px(132.0));

        app.world_mut()
            .entity_mut(cell)
            .insert(GlobalTransform::from_translation(Vec3::new(0.0, 0.0, 2.0)));
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(label_entity).unwrap(),
            Visibility::Hidden
        );

        app.world_mut()
            .entity_mut(cell)
            .insert(GlobalTransform::from_translation(Vec3::new(0.0, 0.0, 0.5)));
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(label_entity).unwrap(),
            Visibility::Visible
        );
        app.world_mut().entity_mut(world_camera).despawn();
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(label_entity).unwrap(),
            Visibility::Hidden
        );
    }

    #[test]
    fn map_exit_removes_scribble_presentation() {
        let mut app = app();
        app.world_mut()
            .spawn((ScribbleMessage("Hello".into()), GlobalTransform::default()));
        app.update();
        let world = app.world_mut();
        assert_eq!(world.query::<&ScribbleLabel>().iter(world).count(), 1);
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Loading);
        app.update();
        let world = app.world_mut();
        assert_eq!(world.query::<&ScribbleLabel>().iter(world).count(), 0);
    }
}
