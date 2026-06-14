//! Hover nameplates: a screen-space label above any hovered entity with an
//! `EntityName`, plus a persistent one for the local player. Spawned on
//! `EntityHoverEntered`, despawned on `EntityHoverExited` (persistent ones survive
//! hover-out); positioned each frame by projecting the target's world position.

use bevy::prelude::*;
use game_engine::core::state::GameState;
use game_engine::domain::entities::components::EntityName;
use game_engine::domain::entities::hover::{EntityHoverEntered, EntityHoverExited};
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
        app.add_observer(on_hover_entered);
        app.add_observer(on_hover_exited);
        app.add_systems(
            Update,
            (ensure_local_player_nameplate, follow_targets).run_if(in_state(GameState::InGame)),
        );
        app.add_systems(OnExit(GameState::InGame), despawn_all_nameplates);
    }
}

#[derive(Component)]
struct Nameplate {
    target: Entity,
    /// Persistent nameplates (the local player) survive hover-out.
    persistent: bool,
}

fn has_nameplate(nameplates: &Query<&Nameplate>, target: Entity) -> bool {
    nameplates.iter().any(|plate| plate.target == target)
}

fn spawn_nameplate(
    commands: &mut Commands,
    font: &WorldspaceFont,
    target: Entity,
    name: &str,
    persistent: bool,
) {
    let color = if persistent {
        theme::ENERGETIC_GREEN
    } else {
        theme::ASHEN_WHITE
    };
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
        Nameplate { target, persistent },
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

fn on_hover_entered(
    trigger: On<EntityHoverEntered>,
    mut commands: Commands,
    names: Query<&EntityName>,
    nameplates: Query<&Nameplate>,
    font: Res<WorldspaceFont>,
) {
    let target = trigger.entity;
    let Ok(name) = names.get(target) else {
        return;
    };
    if has_nameplate(&nameplates, target) {
        return;
    }
    spawn_nameplate(&mut commands, &font, target, &name.name, false);
}

fn on_hover_exited(
    trigger: On<EntityHoverExited>,
    mut commands: Commands,
    nameplates: Query<(Entity, &Nameplate)>,
) {
    let target = trigger.entity;
    for (entity, plate) in &nameplates {
        if plate.target == target && !plate.persistent {
            commands.entity(entity).despawn();
        }
    }
}

fn ensure_local_player_nameplate(
    mut commands: Commands,
    players: Query<(Entity, &EntityName), With<LocalPlayer>>,
    nameplates: Query<&Nameplate>,
    font: Res<WorldspaceFont>,
) {
    let Ok((entity, name)) = players.single() else {
        return;
    };
    if has_nameplate(&nameplates, entity) {
        return;
    }
    spawn_nameplate(&mut commands, &font, entity, &name.name, true);
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
        app.add_observer(on_hover_entered);
        app.add_observer(on_hover_exited);
        app
    }

    #[test]
    fn hover_enter_spawns_named_plate_then_exit_despawns_it() {
        let mut app = test_app();
        let target = app
            .world_mut()
            .spawn(EntityName::new("Poring".to_string()))
            .id();

        app.world_mut().trigger(EntityHoverEntered {
            entity: target,
            entity_id: 1,
        });
        app.world_mut().flush();

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

        app.world_mut().trigger(EntityHoverExited { entity: target });
        app.world_mut().flush();

        let world = app.world_mut();
        assert_eq!(world.query::<&Nameplate>().iter(world).count(), 0);
    }

    #[test]
    fn hover_on_unnamed_entity_spawns_nothing() {
        let mut app = test_app();
        let target = app.world_mut().spawn_empty().id();

        app.world_mut().trigger(EntityHoverEntered {
            entity: target,
            entity_id: 2,
        });
        app.world_mut().flush();

        let world = app.world_mut();
        assert_eq!(world.query::<&Nameplate>().iter(world).count(), 0);
    }
}
