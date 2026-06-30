use bevy::prelude::*;

use super::components::FloorItem;
use crate::domain::entities::hover::{HoverConfig, Hoverable};
use crate::domain::input::{
    CursorChangeRequest, CursorType, ForwardedCursorPosition, TerrainRaycastCache,
};

/// Query filter for the game 3D camera, excluding the equipment-window preview
/// camera. Mirrors `entities::hover_system::GameCameraFilter`.
type GameCameraFilter = (
    With<Camera3d>,
    Without<crate::domain::entities::billboard::EquipmentPreviewCamera>,
);

/// The floor item currently under the cursor, if any. Floor items are not
/// `NetworkEntity`, so they cannot ride `entities::hover::CurrentlyHoveredEntity`.
#[derive(Resource, Default)]
pub struct HoveredFloorItem(pub Option<Entity>);

/// Projects each floor item's world position into screen space and stores the
/// hit-test rect, mirroring `entities::hover_system::update_entity_bounds_system`.
pub fn update_floor_item_bounds(
    mut commands: Commands,
    hover_config: Res<HoverConfig>,
    camera_query: Query<(&Camera, &GlobalTransform), GameCameraFilter>,
    item_query: Query<(Entity, &FloorItem, &GlobalTransform)>,
) {
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    for (entity, _floor_item, item_transform) in item_query.iter() {
        let world_pos = item_transform.translation();

        let Ok(screen_pos) = camera.world_to_viewport(camera_transform, world_pos) else {
            continue;
        };

        let screen_bounds =
            Rect::from_center_size(screen_pos, Vec2::splat(hover_config.radius * 2.0));

        commands
            .entity(entity)
            .try_insert(Hoverable::new(screen_bounds));
    }
}

/// Finds the floor item under the cursor and updates `HoveredFloorItem`,
/// switching the cursor to the pickup icon on entry and restoring it on exit
/// (same walkability check `entities::cursor_interaction::on_entity_hover_exited`
/// uses), mirroring `entities::hover_system::entity_hover_detection_system`.
pub fn floor_item_hover_detection(
    cursor_pos: Res<ForwardedCursorPosition>,
    hoverable_query: Query<(Entity, &FloorItem, &Hoverable)>,
    mut hovered: ResMut<HoveredFloorItem>,
    terrain_cache: Res<TerrainRaycastCache>,
    mut cursor_messages: MessageWriter<CursorChangeRequest>,
) {
    let current = cursor_pos.position.and_then(|cursor_position| {
        hoverable_query
            .iter()
            .find(|(_, _, hoverable)| hoverable.contains_point(cursor_position))
            .map(|(entity, ..)| entity)
    });

    if current == hovered.0 {
        return;
    }

    let cursor_type = match current {
        // TODO: dedicated pickup cursor; `Add` is the closest existing match.
        Some(_) => CursorType::Add,
        None if terrain_cache.is_walkable => CursorType::Default,
        None => CursorType::Impossible,
    };
    cursor_messages.write(CursorChangeRequest::new(cursor_type));

    hovered.0 = current;
}
