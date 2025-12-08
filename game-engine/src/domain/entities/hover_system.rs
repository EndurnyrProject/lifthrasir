use crate::core::state::GameState;
use crate::domain::input::ForwardedCursorPosition;
use crate::domain::system_sets::EntityInteractionSystems;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

use super::{
    components::{NetworkEntity, PendingDespawn},
    hover::{EntityHoverEntered, EntityHoverExited, HoverConfig, Hoverable, HoveredEntity},
    sprite_rendering::components::SpriteObjectTree,
};

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
    camera_query: Query<(&Camera, &GlobalTransform)>,
    entity_query: Query<(Entity, &NetworkEntity, &SpriteObjectTree), Without<PendingDespawn>>,
    sprite_query: Query<&GlobalTransform>,
) {
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    for (entity, _network_entity, sprite_tree) in entity_query.iter() {
        let Ok(sprite_transform) = sprite_query.get(sprite_tree.root) else {
            continue;
        };

        let world_pos = sprite_transform.translation();

        let Some(ndc) = camera.world_to_ndc(camera_transform, world_pos) else {
            continue;
        };

        // Use physical viewport size to match cursor coordinates (which are in physical pixels)
        let Some(viewport_size) = camera.physical_viewport_size() else {
            continue;
        };
        let viewport_size = viewport_size.as_vec2();

        let screen_x = (ndc.x + 1.0) * 0.5 * viewport_size.x;
        let screen_y = (1.0 - ndc.y) * 0.5 * viewport_size.y;

        let screen_bounds = Rect::from_center_size(
            Vec2::new(screen_x, screen_y),
            Vec2::splat(hover_config.radius * 2.0),
        );

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
    mut previous_hovered: Local<Option<Entity>>,
) {
    let Some(cursor_position) = cursor_pos.position else {
        if let Some(prev_entity) = *previous_hovered {
            commands.entity(prev_entity).try_remove::<HoveredEntity>();
            commands.trigger(EntityHoverExited {
                entity: prev_entity,
            });
            *previous_hovered = None;
        }
        return;
    };

    let current_hovered = hoverable_query
        .iter()
        .find(|(_, _, hoverable)| hoverable.contains_point(cursor_position))
        .map(|(entity, network_entity, _)| (entity, network_entity));

    match (current_hovered, *previous_hovered) {
        (Some((entity, network_entity)), Some(prev_entity)) if entity != prev_entity => {
            commands.entity(prev_entity).try_remove::<HoveredEntity>();
            commands.trigger(EntityHoverExited {
                entity: prev_entity,
            });

            commands.entity(entity).insert(HoveredEntity);
            commands.trigger(EntityHoverEntered {
                entity,
                entity_id: network_entity.aid,
                object_type: network_entity.object_type,
            });

            *previous_hovered = Some(entity);
        }
        (Some((entity, network_entity)), None) => {
            commands.entity(entity).insert(HoveredEntity);
            commands.trigger(EntityHoverEntered {
                entity,
                entity_id: network_entity.aid,
                object_type: network_entity.object_type,
            });
            debug!(
                "🎯 Entity hover ENTERED: {:?} (AID: {})",
                entity, network_entity.aid
            );

            *previous_hovered = Some(entity);
        }
        (None, Some(prev_entity)) => {
            commands.entity(prev_entity).try_remove::<HoveredEntity>();
            commands.trigger(EntityHoverExited {
                entity: prev_entity,
            });

            *previous_hovered = None;
        }
        _ => {}
    }
}
