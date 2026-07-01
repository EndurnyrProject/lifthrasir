use crate::{
    core::state::GameState,
    domain::{
        entities::{
            components::NetworkEntity,
            hover::CurrentlyHoveredEntity,
            markers::{LocalPlayer, Mob},
            movement::events::MovementRequested,
            pathfinding::{find_path, CurrentMapPathfindingGrid, WalkablePath},
        },
        system_sets::InputSystems,
        world::components::MapLoader,
    },
    infrastructure::assets::loaders::RoGroundAsset,
    utils::coordinates::world_position_to_spawn_coords,
};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use leafwing_input_manager::prelude::ActionState;
use net_contract::commands::{
    AttackRequested, LearnSkillRequested, SitToggled, StatRaiseRequested,
};

use crate::domain::entities::character::events::{SkillLearnRequested, StatIncreaseRequested};
use crate::domain::entities::character::states::AnimationState;

use super::{
    cursor::CursorType, events::CursorChangeRequest, targeting::TargetingMode,
    terrain_raycast::TerrainRaycastCache, ui_focus::ui_unfocused, ForwardedMouseClick, PlayerAction,
};

// =============================================================================
// PHASE 0.2: UPDATED TO USE FLAT ENTITY STRUCTURE
// =============================================================================
// Removed SpriteObjectTree dependency - queries entity Transform directly.
// =============================================================================

#[derive(SystemParam)]
pub struct MapData<'w, 's> {
    map_loader_query: Query<'w, 's, &'static MapLoader>,
    ground_assets: Res<'w, Assets<RoGroundAsset>>,
    pathfinding_grid: Option<Res<'w, CurrentMapPathfindingGrid>>,
}

#[auto_add_system(
    plugin = crate::app::input_plugin::InputPlugin,
    schedule = Update,
    config(
        in_set = InputSystems::Cursor,
        run_if = in_state(GameState::InGame)
    )
)]
pub fn render_terrain_cursor(mut gizmos: Gizmos, cache: Res<TerrainRaycastCache>) {
    if !cache.is_walkable {
        return;
    }

    let Some(world_pos) = cache.world_position else {
        return;
    };

    let Some((cell_x, cell_y)) = cache.cell_coords else {
        return;
    };

    const RO_UNITS_PER_CELL: f32 = 5.0;
    const HALF_RO_CELL: f32 = RO_UNITS_PER_CELL / 2.0;
    let cell_center_x = cell_x as f32 * RO_UNITS_PER_CELL;
    let cell_center_z = cell_y as f32 * RO_UNITS_PER_CELL;

    const MARKER_SIZE: f32 = 0.4;
    let color = Srgba::hex("00FF00").unwrap().with_alpha(0.4);

    let corners = [
        Vec3::new(
            cell_center_x - HALF_RO_CELL,
            world_pos.y,
            cell_center_z - HALF_RO_CELL,
        ),
        Vec3::new(
            cell_center_x + HALF_RO_CELL,
            world_pos.y,
            cell_center_z - HALF_RO_CELL,
        ),
        Vec3::new(
            cell_center_x - HALF_RO_CELL,
            world_pos.y,
            cell_center_z + HALF_RO_CELL,
        ),
        Vec3::new(
            cell_center_x + HALF_RO_CELL,
            world_pos.y,
            cell_center_z + HALF_RO_CELL,
        ),
    ];

    for corner in corners {
        gizmos.sphere(Isometry3d::from_translation(corner), MARKER_SIZE, color);
    }
}

#[auto_add_system(
    plugin = crate::app::input_plugin::InputPlugin,
    schedule = Update,
    config(
        in_set = InputSystems::Click,
        run_if = in_state(GameState::InGame),
        before = handle_terrain_click
    )
)]
pub fn handle_entity_click(
    mut mouse_click: ResMut<ForwardedMouseClick>,
    targeting: Res<TargetingMode>,
    currently_hovered: Res<CurrentlyHoveredEntity>,
    mob_query: Query<&NetworkEntity, With<Mob>>,
    mut attacks: MessageWriter<AttackRequested>,
) {
    // While a skill is armed, the click belongs to `targeting_click`; consuming
    // it here would turn a skill cast into an auto-attack (order-independent guard).
    if *targeting != TargetingMode::Idle {
        return;
    }

    if mouse_click.position.is_none() {
        return;
    }

    let Some(hovered_entity) = currently_hovered.entity else {
        return;
    };

    let Ok(network_entity) = mob_query.get(hovered_entity) else {
        return;
    };

    let target_gid = network_entity.gid;
    debug!("Attacking mob with GID: {}", target_gid);

    attacks.write(AttackRequested {
        target_id: target_gid,
    });

    mouse_click.position.take();
}

#[auto_add_system(
    plugin = crate::app::input_plugin::InputPlugin,
    schedule = Update,
    config(
        in_set = InputSystems::Click,
        run_if = in_state(GameState::InGame)
    )
)]
pub fn handle_terrain_click(
    mut commands: Commands,
    mut mouse_click: ResMut<ForwardedMouseClick>,
    targeting: Res<TargetingMode>,
    cache: Res<TerrainRaycastCache>,
    map_data: MapData,
    player_query: Query<(Entity, &Transform), With<LocalPlayer>>,
) {
    // A click while a skill is armed must not move the player: leave it for
    // `targeting_click` to resolve into a cast (order-independent guard).
    if *targeting != TargetingMode::Idle {
        return;
    }

    if mouse_click.position.take().is_none() {
        return;
    }

    let Some((dest_x, dest_y)) = cache.cell_coords else {
        debug!("Click with no valid raycast cache");
        return;
    };

    let Ok(map_loader) = map_data.map_loader_query.single() else {
        warn!("No map loaded, ignoring terrain click");
        return;
    };

    let Some(ground_asset) = map_data.ground_assets.get(&map_loader.ground) else {
        warn!("Ground asset not loaded, ignoring terrain click");
        return;
    };

    let Ok((player_entity, transform)) = player_query.single() else {
        warn!("No player character found for movement request");
        return;
    };

    let current_pos = transform.translation;
    let (current_x, current_y) = world_position_to_spawn_coords(
        current_pos,
        ground_asset.ground.width,
        ground_asset.ground.height,
    );

    let Some(grid) = map_data.pathfinding_grid else {
        warn!("Pathfinding grid not yet loaded, ignoring terrain click");
        return;
    };

    let path = find_path(&grid.0, (current_x, current_y), (dest_x, dest_y));

    match path {
        Some(waypoints) if waypoints.len() > 1 => {
            debug!("Path found with {} waypoints", waypoints.len());

            commands
                .entity(player_entity)
                .insert(WalkablePath::new(waypoints.clone(), (dest_x, dest_y)));

            commands.trigger(MovementRequested {
                entity: player_entity,
                dest_x,
                dest_y,
                direction: 0,
            });

            debug!(
                "Terrain clicked: current=({}, {}), final destination=({}, {}), path length={}",
                current_x,
                current_y,
                dest_x,
                dest_y,
                waypoints.len()
            );
        }
        Some(_waypoints) => {
            debug!("Direct path (adjacent or same cell)");
            commands.trigger(MovementRequested {
                entity: player_entity,
                dest_x,
                dest_y,
                direction: 0,
            });

            debug!(
                "Terrain clicked: direct movement from ({}, {}) to ({}, {})",
                current_x, current_y, dest_x, dest_y
            );
        }
        None => {
            warn!("No path found to ({}, {})", dest_x, dest_y);
        }
    }
}

#[auto_add_system(
    plugin = crate::app::input_plugin::InputPlugin,
    schedule = Update,
    config(
        in_set = InputSystems::Click,
        run_if = in_state(GameState::InGame)
    )
)]
pub fn update_cursor_for_terrain(
    cache: Res<TerrainRaycastCache>,
    currently_hovered: Res<CurrentlyHoveredEntity>,
    mut cursor_messages: MessageWriter<CursorChangeRequest>,
) {
    if currently_hovered.entity.is_some() {
        return;
    }

    let cursor_type = if cache.is_walkable {
        CursorType::Default
    } else {
        CursorType::Impossible
    };

    cursor_messages.write(CursorChangeRequest::new(cursor_type));
}

#[auto_add_system(
    plugin = crate::app::input_plugin::InputPlugin,
    schedule = Update,
    config(
        in_set = InputSystems::Click,
        run_if = in_state(GameState::InGame).and_then(ui_unfocused)
    )
)]
pub fn handle_sit_toggle(
    player: Query<(&ActionState<PlayerAction>, &AnimationState), With<LocalPlayer>>,
    mut sits: MessageWriter<SitToggled>,
) {
    let Ok((actions, anim)) = player.single() else {
        return;
    };

    if !actions.just_pressed(&PlayerAction::Sit) {
        return;
    }

    sits.write(SitToggled {
        sit: *anim != AnimationState::Sitting,
    });
}

#[auto_add_system(
    plugin = crate::app::input_plugin::InputPlugin,
    schedule = Update,
    config(run_if = in_state(GameState::InGame))
)]
pub fn handle_stat_increase_requests(
    mut requests: MessageReader<StatIncreaseRequested>,
    mut stat_raises: MessageWriter<StatRaiseRequested>,
) {
    for request in requests.read() {
        stat_raises.write(StatRaiseRequested {
            stat_id: request.status_id as u32,
            amount: request.amount as u32,
        });
    }
}

#[auto_add_system(
    plugin = crate::app::input_plugin::InputPlugin,
    schedule = Update,
    config(run_if = in_state(GameState::InGame))
)]
pub fn handle_learn_skill_requests(
    mut requests: MessageReader<SkillLearnRequested>,
    mut learns: MessageWriter<LearnSkillRequested>,
) {
    for request in requests.read() {
        learns.write(LearnSkillRequested {
            skill_id: request.skill_id,
        });
    }
}

#[auto_add_system(
    plugin = crate::app::input_plugin::InputPlugin,
    schedule = OnEnter(GameState::Login)
)]
pub fn set_default_cursor_for_login(mut cursor_messages: MessageWriter<CursorChangeRequest>) {
    cursor_messages.write(CursorChangeRequest::new(CursorType::Default));
}

#[auto_add_system(
    plugin = crate::app::input_plugin::InputPlugin,
    schedule = OnEnter(GameState::ServerSelection)
)]
pub fn set_default_cursor_for_server_selection(
    mut cursor_messages: MessageWriter<CursorChangeRequest>,
) {
    cursor_messages.write(CursorChangeRequest::new(CursorType::Default));
}

#[auto_add_system(
    plugin = crate::app::input_plugin::InputPlugin,
    schedule = OnEnter(GameState::CharacterSelection)
)]
pub fn set_default_cursor_for_character_selection(
    mut cursor_messages: MessageWriter<CursorChangeRequest>,
) {
    cursor_messages.write(CursorChangeRequest::new(CursorType::Default));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::types::ObjectType;

    fn click_app() -> App {
        let mut app = App::new();
        app.init_resource::<ForwardedMouseClick>()
            .init_resource::<TargetingMode>()
            .init_resource::<CurrentlyHoveredEntity>()
            .add_message::<AttackRequested>()
            .add_systems(Update, handle_entity_click);
        app
    }

    fn hovered_mob(app: &mut App) -> Entity {
        let mob = app
            .world_mut()
            .spawn((NetworkEntity::new(42, 42, ObjectType::Mob), Mob))
            .id();
        app.world_mut()
            .resource_mut::<CurrentlyHoveredEntity>()
            .entity = Some(mob);
        app.world_mut()
            .resource_mut::<ForwardedMouseClick>()
            .position = Some(Vec2::ZERO);
        mob
    }

    fn attacks(app: &App) -> Vec<AttackRequested> {
        app.world()
            .resource::<Messages<AttackRequested>>()
            .iter_current_update_messages()
            .cloned()
            .collect()
    }

    #[test]
    fn armed_targeting_leaves_click_for_targeting_and_does_not_attack() {
        let mut app = click_app();
        hovered_mob(&mut app);
        *app.world_mut().resource_mut::<TargetingMode>() =
            TargetingMode::AwaitingEntity { skill_id: 5, level: 1 };

        app.update();

        assert!(attacks(&app).is_empty(), "armed skill must not auto-attack");
        assert!(
            app.world().resource::<ForwardedMouseClick>().position.is_some(),
            "click must survive for targeting_click to resolve into a cast"
        );
    }

    #[test]
    fn idle_targeting_attacks_hovered_mob_and_consumes_click() {
        let mut app = click_app();
        hovered_mob(&mut app);

        app.update();

        let msgs = attacks(&app);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].target_id, 42);
        assert!(
            app.world().resource::<ForwardedMouseClick>().position.is_none(),
            "a normal attack consumes the click"
        );
    }
}
