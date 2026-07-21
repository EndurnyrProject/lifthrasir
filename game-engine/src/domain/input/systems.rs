use crate::{
    core::state::GameState,
    domain::{
        entities::{
            hover::CurrentlyHoveredEntity,
            markers::LocalPlayer,
            movement::events::MovementRequested,
            pathfinding::{CurrentMapPathfindingGrid, WalkablePath, find_path},
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
use net_contract::commands::{LearnSkillRequested, SitToggled, StatRaiseRequested};

use crate::domain::entities::character::events::{SkillLearnRequested, StatIncreaseRequested};
use crate::domain::entities::character::states::AnimationState;

use super::{
    ForwardedMouseClick, LockedTarget, PlayerAction, cursor::CursorType,
    events::CursorChangeRequest, targeting::TargetingMode, terrain_raycast::TerrainRaycastCache,
    ui_focus::ui_unfocused,
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
    mut locked_target: ResMut<LockedTarget>,
) {
    // A click while a skill is armed must not move the player: leave it for
    // `targeting_click` to resolve into a cast (order-independent guard).
    if *targeting != TargetingMode::Idle {
        return;
    }

    if mouse_click.position.take().is_none() {
        return;
    }

    // A move command disengages any locked attack target.
    *locked_target = LockedTarget::default();

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
