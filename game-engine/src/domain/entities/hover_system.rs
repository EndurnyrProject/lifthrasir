use crate::core::state::GameState;
use crate::domain::input::ForwardedCursorPosition;
use crate::domain::sprite::tags::SPRITE_BASE_Y_OFFSET;
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

/// Query filter for the game 3D camera, excluding the equipment-window preview
/// camera (a 2D UI camera also exists, so `With<Camera3d>` disambiguates).
type GameCameraFilter = (
    With<Camera3d>,
    Without<crate::domain::entities::billboard::EquipmentPreviewCamera>,
);

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
    camera_query: Query<(&Camera, &GlobalTransform), GameCameraFilter>,
    entity_query: Query<(Entity, &NetworkEntity, &GlobalTransform), Without<PendingDespawn>>,
) {
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    for (entity, _network_entity, entity_transform) in entity_query.iter() {
        let Some(screen_bounds) = sprite_hover_bounds(
            camera,
            camera_transform,
            entity_transform.translation(),
            hover_config.radius,
        ) else {
            continue;
        };

        commands
            .entity(entity)
            .try_insert(Hoverable::new(screen_bounds));
    }
}

/// Screen-space hit box centred on the *rendered* sprite, which the billboard
/// child lifts by `SPRITE_BASE_Y_OFFSET` (world-up is -Y). Anchoring on the
/// un-lifted root left the box below the visible sprite so clicks missed it.
///
/// Logical viewport pixels to match `ForwardedCursorPosition` (CursorMoved is
/// logical); `world_to_viewport` projects into logical space so the cursor and
/// box share units regardless of window scale factor.
fn sprite_hover_bounds(
    camera: &Camera,
    camera_transform: &GlobalTransform,
    root_translation: Vec3,
    radius: f32,
) -> Option<Rect> {
    let sprite_pos = root_translation + Vec3::Y * SPRITE_BASE_Y_OFFSET;
    let screen_pos = camera
        .world_to_viewport(camera_transform, sprite_pos)
        .ok()?;
    Some(Rect::from_center_size(
        screen_pos,
        Vec2::splat(radius * 2.0),
    ))
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::{PerspectiveProjection, Projection, RenderTargetInfo};

    fn test_camera(size: Vec2) -> Camera {
        let mut projection = Projection::Perspective(PerspectiveProjection::default());
        projection.update(size.x, size.y);
        let mut camera = Camera::default();
        camera.computed.target_info = Some(RenderTargetInfo {
            physical_size: size.as_uvec2(),
            scale_factor: 1.0,
        });
        camera.computed.clip_from_view = projection.get_clip_from_view();
        camera
    }

    #[test]
    fn hover_box_covers_rendered_sprite() {
        let size = Vec2::new(1280.0, 720.0);
        let camera = test_camera(size);
        let camera_transform = GlobalTransform::from(
            Transform::from_xyz(0.0, 0.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
        );
        let root_translation = Vec3::new(3.0, 0.0, 0.0);

        let bounds =
            sprite_hover_bounds(&camera, &camera_transform, root_translation, 45.0).unwrap();

        let sprite_pos = root_translation + Vec3::Y * SPRITE_BASE_Y_OFFSET;
        let sprite_screen = camera
            .world_to_viewport(&camera_transform, sprite_pos)
            .unwrap();
        assert!(
            bounds.contains(sprite_screen),
            "cursor on the rendered sprite must fall inside the hover box"
        );

        // The lift is load-bearing: the box must not stay centred on the un-lifted root.
        let root_screen = camera
            .world_to_viewport(&camera_transform, root_translation)
            .unwrap();
        assert!(bounds.center().abs_diff_eq(sprite_screen, 0.01));
        assert!(!bounds.center().abs_diff_eq(root_screen, 0.01));
    }
}
