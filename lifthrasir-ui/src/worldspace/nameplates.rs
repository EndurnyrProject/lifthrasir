//! Hover nameplates: a screen-space label above the currently hovered entity (any
//! entity with an `EntityName`, including the local player). Driven each frame by the
//! `HoveredEntity` marker so it picks up names that arrive asynchronously after the
//! on-hover server name request; positioned by projecting the target's world position.

use bevy::prelude::*;
use game_engine::core::state::GameState;
use game_engine::domain::entities::components::EntityName;
use game_engine::domain::entities::hover::HoveredEntity;
use game_engine::domain::entities::markers::LocalPlayer;

use crate::theme;
use crate::worldspace::WorldspaceFont;

const NAMEPLATE_WIDTH: f32 = 220.0;
const NAMEPLATE_FONT_SIZE: f32 = 14.0;
/// Pixels above the entity's projected origin. ponytail: fixed screen offset, not
/// zoom-scaled — tune live via BRP if it drifts off the sprite's head.
const NAMEPLATE_SCREEN_OFFSET_Y: f32 = 44.0;
/// Above the world camera, below the fade overlay (`i32::MAX - 1`) and cursor.
const NAMEPLATE_Z: i32 = 100;

pub struct NameplatePlugin;

impl Plugin for NameplatePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (sync_nameplates, follow_targets).run_if(in_state(GameState::InGame)),
        );
        app.add_systems(OnExit(GameState::InGame), despawn_all_nameplates);
    }
}

#[derive(Component)]
struct Nameplate {
    target: Entity,
}

fn has_nameplate(nameplates: &Query<&Nameplate>, target: Entity) -> bool {
    nameplates.iter().any(|plate| plate.target == target)
}

fn spawn_nameplate(
    commands: &mut Commands,
    font: &WorldspaceFont,
    target: Entity,
    name: &str,
    is_self: bool,
) {
    let color = if is_self { theme::EMERALD } else { theme::TEXT };
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(NAMEPLATE_WIDTH),
            justify_content: JustifyContent::Center,
            ..default()
        },
        GlobalZIndex(NAMEPLATE_Z),
        Visibility::Hidden,
        Pickable::IGNORE,
        Nameplate { target },
        children![(
            Text::new(name),
            TextFont {
                font: font.0.clone(),
                font_size: NAMEPLATE_FONT_SIZE,
                ..default()
            },
            TextColor(color),
            Pickable::IGNORE,
        )],
    ));
}

/// Keep one nameplate per hovered, named entity. Runs every frame so it catches
/// `EntityName`s that arrive after the on-hover server name request resolves.
fn sync_nameplates(
    mut commands: Commands,
    hovered: Query<(Entity, &EntityName), With<HoveredEntity>>,
    local_player: Query<(), With<LocalPlayer>>,
    nameplates: Query<&Nameplate>,
    stale: Query<(Entity, &Nameplate)>,
    still_hovered: Query<(), With<HoveredEntity>>,
    font: Res<WorldspaceFont>,
) {
    for (target, name) in &hovered {
        if has_nameplate(&nameplates, target) {
            continue;
        }
        let is_self = local_player.get(target).is_ok();
        spawn_nameplate(&mut commands, &font, target, &name.name, is_self);
    }

    for (entity, plate) in &stale {
        if still_hovered.get(plate.target).is_err() {
            commands.entity(entity).despawn();
        }
    }
}

fn follow_targets(
    camera: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    targets: Query<&GlobalTransform>,
    mut nameplates: Query<(Entity, &Nameplate, &mut Node, &mut Visibility)>,
    mut commands: Commands,
) {
    let Ok((camera, camera_transform)) = camera.single() else {
        return;
    };
    for (entity, plate, mut node, mut visibility) in &mut nameplates {
        let Ok(target_transform) = targets.get(plate.target) else {
            commands.entity(entity).despawn();
            continue;
        };
        match camera.world_to_viewport(camera_transform, target_transform.translation()) {
            Ok(screen) => {
                node.left = Val::Px(screen.x - NAMEPLATE_WIDTH / 2.0);
                node.top = Val::Px(screen.y - NAMEPLATE_SCREEN_OFFSET_Y);
                *visibility = Visibility::Visible;
            }
            Err(_) => *visibility = Visibility::Hidden,
        }
    }
}

fn despawn_all_nameplates(mut commands: Commands, nameplates: Query<Entity, With<Nameplate>>) {
    for entity in &nameplates {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(WorldspaceFont(Handle::default()));
        app.add_systems(Update, sync_nameplates);
        app
    }

    fn plate_count(app: &mut App) -> usize {
        let world = app.world_mut();
        world.query::<&Nameplate>().iter(world).count()
    }

    #[test]
    fn hovered_named_entity_gets_plate_then_loses_it_on_unhover() {
        let mut app = test_app();
        let target = app
            .world_mut()
            .spawn((EntityName::new("Poring".to_string()), HoveredEntity))
            .id();

        app.update();

        let world = app.world_mut();
        let plates: Vec<&Nameplate> = world.query::<&Nameplate>().iter(world).collect();
        assert_eq!(plates.len(), 1);
        assert_eq!(plates[0].target, target);
        let label = world
            .query::<&Text>()
            .iter(world)
            .map(|text| text.0.clone())
            .next();
        assert_eq!(label.as_deref(), Some("Poring"));

        app.world_mut()
            .entity_mut(target)
            .remove::<HoveredEntity>();
        app.update();

        assert_eq!(plate_count(&mut app), 0);
    }

    #[test]
    fn hovered_unnamed_entity_spawns_nothing() {
        let mut app = test_app();
        app.world_mut().spawn(HoveredEntity);

        app.update();

        assert_eq!(plate_count(&mut app), 0);
    }

    #[test]
    fn plate_appears_once_name_arrives_while_still_hovered() {
        let mut app = test_app();
        let target = app.world_mut().spawn(HoveredEntity).id();

        app.update();
        assert_eq!(plate_count(&mut app), 0);

        app.world_mut()
            .entity_mut(target)
            .insert(EntityName::new("Poring".to_string()));
        app.update();

        assert_eq!(plate_count(&mut app), 1);
    }
}
