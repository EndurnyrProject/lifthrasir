//! Hover nameplates: a screen-space label at the feet of the currently hovered entity
//! (any entity with an `EntityName`, including the local player). Driven each frame by
//! the `HoveredEntity` marker so it picks up names that arrive asynchronously after the
//! on-hover server name request; positioned by projecting the target's world position.

use bevy::prelude::*;
use game_engine::domain::entities::components::EntityName;
use game_engine::domain::entities::hover::HoveredEntity;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::domain::entities::EntityRegistry;
use game_engine::domain::party::PartyState;
use game_engine::core::state::GameState;

use crate::theme;
use crate::worldspace::{viewport_to_ui, WorldCameraFilter, WorldspaceFont};

const NAMEPLATE_WIDTH: f32 = 220.0;
const NAMEPLATE_FONT_SIZE: f32 = 13.0;
/// The party-name line above the character name; smaller than the name and gold-tinted.
const PARTY_FONT_SIZE: f32 = 11.0;
/// Pixels below the entity's projected origin (the feet). Classic RO shows the name at
/// the character's feet. NOTE: fixed screen offset, not zoom-scaled — tune live via
/// BRP if it drifts off the sprite's feet.
const NAMEPLATE_FOOT_GAP: f32 = 6.0;
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

/// The party name to show for `target`, or `None` when the client is partyless or the
/// hovered entity is not one of its party members. Resolves the entity's char_id via the
/// registry and checks membership, so only actual party members carry the party line.
fn party_name_for<'a>(
    registry: &EntityRegistry,
    party: &'a PartyState,
    target: Entity,
) -> Option<&'a str> {
    if !party.in_party() {
        return None;
    }
    let char_id = registry.get_account_id(target)?;
    party
        .members
        .iter()
        .any(|member| member.char_id == char_id)
        .then_some(party.name.as_str())
}

fn spawn_nameplate(
    commands: &mut Commands,
    font: &WorldspaceFont,
    target: Entity,
    name: &str,
    is_self: bool,
    party: Option<&str>,
) {
    let name_color = if is_self {
        theme::EMERALD_BRI
    } else {
        theme::TEXT
    };
    let pill = commands
        .spawn((
            // Transparent positioning wrapper: a fixed width centered on the entity keeps
            // the content-sized pill horizontally centered regardless of name length.
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
        ))
        .id();

    // Endurnir glass pill: translucent dark fill, gold-faint hairline, rounded. A column
    // so an optional party line stacks above the character name (classic RO ordering).
    let inner = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(9.0)),
                ..default()
            },
            BackgroundColor(theme::GLASS),
            BorderColor::all(theme::GOLD_FAINT),
            Pickable::IGNORE,
            ChildOf(pill),
        ))
        .id();

    if let Some(party) = party {
        commands.spawn((
            Text::new(party),
            TextFont {
                font: font.0.clone().into(),
                font_size: PARTY_FONT_SIZE.into(),
                ..default()
            },
            TextColor(theme::GOLD),
            Pickable::IGNORE,
            ChildOf(inner),
        ));
    }

    commands.spawn((
        Text::new(name),
        TextFont {
            font: font.0.clone().into(),
            font_size: NAMEPLATE_FONT_SIZE.into(),
            ..default()
        },
        TextColor(name_color),
        Pickable::IGNORE,
        ChildOf(inner),
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
    registry: Res<EntityRegistry>,
    party: Res<PartyState>,
    font: Res<WorldspaceFont>,
) {
    for (target, name) in &hovered {
        if has_nameplate(&nameplates, target) {
            continue;
        }
        let is_self = local_player.get(target).is_ok();
        // The server's name-all reply carries the unit's party name for any party
        // (ours or another), so prefer it; fall back to our own roster for units named
        // without that reply — notably the local player, spawned without a name request.
        let party_name = name
            .party_name
            .as_deref()
            .or_else(|| party_name_for(&registry, &party, target));
        spawn_nameplate(&mut commands, &font, target, &name.name, is_self, party_name);
    }

    for (entity, plate) in &stale {
        if still_hovered.get(plate.target).is_err() {
            commands.entity(entity).despawn();
        }
    }
}

fn follow_targets(
    camera: Query<(&Camera, &GlobalTransform), WorldCameraFilter>,
    targets: Query<&GlobalTransform>,
    ui_scale: Res<UiScale>,
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
                let pos = viewport_to_ui(screen, &ui_scale);
                node.left = Val::Px(pos.x - NAMEPLATE_WIDTH / 2.0);
                node.top = Val::Px(pos.y + NAMEPLATE_FOOT_GAP);
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
        app.init_resource::<EntityRegistry>();
        app.init_resource::<PartyState>();
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

        app.world_mut().entity_mut(target).remove::<HoveredEntity>();
        app.update();

        assert_eq!(plate_count(&mut app), 0);
    }

    #[test]
    fn party_member_plate_shows_party_name_above_char_name() {
        use game_engine::domain::party::PartyState;
        use net_contract::dto::PartyMemberInfo;

        let mut app = test_app();
        let target = app
            .world_mut()
            .spawn((EntityName::new("Solveig".to_string()), HoveredEntity))
            .id();
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(42, target);
        app.world_mut().insert_resource(PartyState {
            party_id: 7,
            name: "Wolfpack".to_string(),
            members: vec![PartyMemberInfo {
                char_id: 42,
                name: "Solveig".to_string(),
                base_level: 99,
                online: true,
                map: "prontera".to_string(),
            }],
            ..default()
        });

        app.update();

        let world = app.world_mut();
        let labels: Vec<String> = world
            .query::<&Text>()
            .iter(world)
            .map(|text| text.0.clone())
            .collect();
        assert!(labels.contains(&"Wolfpack".to_string()));
        assert!(labels.contains(&"Solveig".to_string()));
    }

    #[test]
    fn other_partys_member_shows_its_party_from_entity_name() {
        let mut app = test_app();
        let mut name = EntityName::new("Rival".to_string());
        name.party_name = Some("Ravens".to_string());
        app.world_mut().spawn((name, HoveredEntity));

        app.update();

        let world = app.world_mut();
        let labels: Vec<String> = world
            .query::<&Text>()
            .iter(world)
            .map(|text| text.0.clone())
            .collect();
        assert!(labels.contains(&"Ravens".to_string()));
        assert!(labels.contains(&"Rival".to_string()));
    }

    #[test]
    fn non_member_plate_has_no_party_line() {
        use game_engine::domain::party::PartyState;

        let mut app = test_app();
        let target = app
            .world_mut()
            .spawn((EntityName::new("Stranger".to_string()), HoveredEntity))
            .id();
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(99, target);
        app.world_mut().insert_resource(PartyState {
            party_id: 7,
            name: "Wolfpack".to_string(),
            ..default()
        });

        app.update();

        let world = app.world_mut();
        let labels: Vec<String> = world
            .query::<&Text>()
            .iter(world)
            .map(|text| text.0.clone())
            .collect();
        assert_eq!(labels, vec!["Stranger".to_string()]);
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
