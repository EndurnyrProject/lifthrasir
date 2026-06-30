use crate::core::state::GameState;
use crate::domain::input::ForwardedCursorPosition;
use crate::domain::system_sets::EntityInteractionSystems;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

use super::{
    components::{NetworkEntity, PendingDespawn},
    hover::{
        CurrentlyHoveredEntity, EntityHoverEntered, EntityHoverExited, HoverConfig, Hoverable,
        HoveredEntity,
    },
};

// =============================================================================
// PHASE 0.2: UPDATED TO USE FLAT ENTITY STRUCTURE
// =============================================================================
// Removed SpriteObjectTree dependency - queries entity GlobalTransform directly.
// =============================================================================

#[auto_add_system(
    plugin = crate::app::entity_hover_plugin::EntityHoverDomainPlugin,
    schedule = Update,
    config(
        in_set = EntityInteractionSystems::Hover,
        run_if = in_state(GameState::InGame)
    )
)]
pub fn update_entity_bounds_system(
    mut commands: Commands,
    hover_config: Res<HoverConfig>,
    // With<Camera3d>: a 2D UI camera also exists, so an unfiltered single() matches
    // two cameras, fails, and the system silently inserts no bounds. Same filter the
    // nameplate's follow_targets uses to pick the game camera.
    camera_query: Query<
        (&Camera, &GlobalTransform),
        (
            With<Camera3d>,
            Without<crate::domain::entities::billboard::EquipmentPreviewCamera>,
        ),
    >,
    entity_query: Query<(Entity, &NetworkEntity, &GlobalTransform), Without<PendingDespawn>>,
) {
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    for (entity, _network_entity, entity_transform) in entity_query.iter() {
        let world_pos = entity_transform.translation();

        // Logical viewport pixels to match ForwardedCursorPosition (CursorMoved is
        // logical). The old world_to_ndc * physical_viewport_size produced physical
        // pixels, mismatching the cursor by the window scale factor (2x on Retina) so
        // hover never registered. Same projection the nameplate uses to follow targets.
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

#[auto_add_system(
    plugin = crate::app::entity_hover_plugin::EntityHoverDomainPlugin,
    schedule = Update,
    config(
        in_set = EntityInteractionSystems::Hover,
        run_if = in_state(GameState::InGame)
    )
)]
pub fn entity_hover_detection_system(
    mut commands: Commands,
    cursor_pos: Res<ForwardedCursorPosition>,
    hoverable_query: Query<(Entity, &NetworkEntity, &Hoverable)>,
    mut currently_hovered: ResMut<CurrentlyHoveredEntity>,
) {
    let Some(cursor_position) = cursor_pos.position else {
        if let Some(prev_entity) = currently_hovered.entity {
            commands.entity(prev_entity).try_remove::<HoveredEntity>();
            commands.trigger(EntityHoverExited {
                entity: prev_entity,
            });
            currently_hovered.entity = None;
        }
        return;
    };

    let current_hovered = hoverable_query
        .iter()
        .find(|(_, _, hoverable)| hoverable.contains_point(cursor_position))
        .map(|(entity, network_entity, _)| (entity, network_entity));

    match (current_hovered, currently_hovered.entity) {
        (Some((entity, network_entity)), Some(prev_entity)) if entity != prev_entity => {
            commands.entity(prev_entity).try_remove::<HoveredEntity>();
            commands.trigger(EntityHoverExited {
                entity: prev_entity,
            });

            commands.entity(entity).try_insert(HoveredEntity);
            commands.trigger(EntityHoverEntered {
                entity,
                entity_id: network_entity.aid,
            });

            currently_hovered.entity = Some(entity);
        }
        (Some((entity, network_entity)), None) => {
            commands.entity(entity).try_insert(HoveredEntity);
            commands.trigger(EntityHoverEntered {
                entity,
                entity_id: network_entity.aid,
            });
            debug!(
                "Entity hover ENTERED: {:?} (AID: {})",
                entity, network_entity.aid
            );

            currently_hovered.entity = Some(entity);
        }
        (None, Some(prev_entity)) => {
            commands.entity(prev_entity).try_remove::<HoveredEntity>();
            commands.trigger(EntityHoverExited {
                entity: prev_entity,
            });

            currently_hovered.entity = None;
        }
        _ => {}
    }
}
