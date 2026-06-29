use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

use crate::domain::entities::character::components::visual::ActionType;
use crate::domain::entities::sprite_rendering::components::{
    BodyAttachPoint, HeadAttachment, HeadLayer, PlayerSprite, RenderLayer,
};
use crate::domain::entities::sprite_rendering::layout::{ActionLayout, PlayerLayout};
use crate::domain::sprite::tags::{layer_order, LAYER_BODY, Z_OFFSET_PER_LAYER};
use crate::domain::system_sets::SpriteRenderingSystems;
use crate::infrastructure::assets::ro_animation_asset::RoAnimationAsset;
use crate::utils::constants::SPRITE_WORLD_SCALE;

type HeadLayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static HeadAttachment,
        &'static RenderLayer,
        &'static ChildOf,
        &'static MeshMaterial3d<StandardMaterial>,
        &'static mut Transform,
    ),
    With<HeadLayer>,
>;

/// Head offset relative to the entity origin in RO screen space, matching the
/// original client: the head layer position is corrected by the difference
/// between the body and head attach points so the necks align.
pub(crate) fn head_screen_offset(
    head_layer_pos: Vec2,
    body_attach: Vec2,
    head_attach: Vec2,
) -> Vec2 {
    head_layer_pos + (body_attach - head_attach)
}

/// Head offset relative to the rendered body layer, in screen space
/// (stored coordinates: X right, Y up).
pub(crate) fn head_billboard_delta(screen_offset: Vec2, body_layer_pos: Vec2) -> Vec2 {
    screen_offset - body_layer_pos
}

/// Attach point and layer position from frame 0 of the body's current action.
fn body_idle_attach(
    animations: &Assets<RoAnimationAsset>,
    body_render_layer: &RenderLayer,
    action_index: usize,
) -> Option<(Vec2, Vec2)> {
    let body_animation = animations.get(&body_render_layer.animation)?;
    let body_action = body_animation.actions.get(action_index)?;
    let frame = body_action.frames.first()?;
    let attach_point = frame.attach_point?;
    let layer_pos = frame.parts.first()?.position;
    Some((attach_point, layer_pos))
}

/// Synchronizes the head layer with the body: texture, scale, and position all
/// come from the same frame so the head follows the body during walk/attack.
/// Idle stays on frame 0 to avoid the doridori head cycle.
///
/// Body and head are separate billboard quads, so the head offset must be
/// applied in billboard space (rotated by the camera rotation). A parent-space
/// offset would be foreshortened by the camera pitch, while distances inside
/// the quad textures are not, detaching the head from the neck.
#[auto_add_system(
    plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::TransformUpdate, after = crate::domain::entities::sprite_rendering::systems::body_sync::sync_player_body_layer)
)]
pub fn sync_player_head_layer(
    animations: Res<Assets<RoAnimationAsset>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    camera_query: Query<&Transform, (With<Camera3d>, Without<HeadLayer>)>,
    parent_query: Query<&PlayerSprite>,
    body_query: Query<(&BodyAttachPoint, &RenderLayer, &Transform), Without<HeadLayer>>,
    mut head_query: HeadLayerQuery,
) {
    let Ok(camera_transform) = camera_query.single() else {
        return;
    };

    for (attachment, head_layer, child_of, material_handle, mut transform) in head_query.iter_mut()
    {
        let Ok((body_attach, body_render_layer, body_transform)) =
            body_query.get(attachment.body_entity)
        else {
            continue;
        };

        let Ok(ro_sprite) = parent_query.get(child_of.parent()) else {
            continue;
        };

        let Some(head_animation) = animations.get(&head_layer.animation) else {
            continue;
        };

        let action_index = PlayerLayout::validate_action_index(
            ro_sprite.action_index(),
            head_animation.actions.len(),
        );
        let Some(head_action) = head_animation.actions.get(action_index) else {
            continue;
        };

        if head_action.frames.is_empty() {
            continue;
        }

        let head_frame_index = if ro_sprite.action_type == ActionType::Idle {
            0
        } else {
            body_attach
                .frame_index
                .min(head_action.frames.len().saturating_sub(1))
        };

        let Some(head_frame) = head_action.frames.get(head_frame_index) else {
            continue;
        };

        let Some(part) = head_frame.parts.first() else {
            continue;
        };

        if let Some(texture) = head_animation.textures.get(part.texture_index) {
            if let Some(mut material) = materials.get_mut(&material_handle.0) {
                material.base_color_texture = Some(texture.clone());
            }
        }

        let mut scale_x = part.scale.x * part.texture_size.x * SPRITE_WORLD_SCALE;
        let scale_y = part.scale.y * part.texture_size.y * SPRITE_WORLD_SCALE;

        if part.mirror {
            scale_x = -scale_x;
        }

        transform.scale = Vec3::new(scale_x, scale_y, 1.0);

        let Some(head_attach) = head_frame.attach_point else {
            continue;
        };

        // During idle the head is pinned to frame 0, so the body attach data
        // must come from frame 0 too: the published attach point cycles
        // through the doridori poses and would make the pinned head twitch.
        let (body_attach_point, body_layer_pos) = if ro_sprite.action_type == ActionType::Idle {
            body_idle_attach(&animations, body_render_layer, action_index)
                .unwrap_or((body_attach.attach_point, body_attach.layer_pos))
        } else {
            (body_attach.attach_point, body_attach.layer_pos)
        };

        let screen_offset = head_screen_offset(part.position, body_attach_point, head_attach);
        let delta = head_billboard_delta(screen_offset, body_layer_pos) * SPRITE_WORLD_SCALE;
        let world_delta = camera_transform.rotation * delta.extend(0.0);

        let layer_gap = (layer_order(head_layer.layer) as f32 - layer_order(LAYER_BODY) as f32)
            * Z_OFFSET_PER_LAYER;

        transform.translation =
            body_transform.translation + world_delta + Vec3::new(0.0, 0.0, layer_gap);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Values from the novice male body/head ACT files (idle south, frame 0),
    // in stored coordinates (Y negated during extraction, so Y is up).
    const BODY_LAYER_POS: Vec2 = Vec2::new(0.0, 25.0);
    const BODY_ATTACH: Vec2 = Vec2::new(1.0, 56.0);
    const HEAD_LAYER_POS: Vec2 = Vec2::new(-1.0, 67.0);
    const HEAD_ATTACH: Vec2 = Vec2::new(1.0, 56.0);

    #[test]
    fn test_head_screen_offset_aligns_necks() {
        assert_eq!(
            head_screen_offset(HEAD_LAYER_POS, BODY_ATTACH, HEAD_ATTACH),
            Vec2::new(-1.0, 67.0)
        );
    }

    #[test]
    fn test_head_screen_offset_identity_when_attach_points_match() {
        let head_layer_pos = Vec2::new(3.0, 5.0);
        let attach = Vec2::new(2.0, -20.0);

        assert_eq!(
            head_screen_offset(head_layer_pos, attach, attach),
            head_layer_pos
        );
    }

    #[test]
    fn test_head_billboard_delta_keeps_screen_gap() {
        let screen_offset = head_screen_offset(HEAD_LAYER_POS, BODY_ATTACH, HEAD_ATTACH);
        let delta = head_billboard_delta(screen_offset, BODY_LAYER_POS);

        // The head must sit 42 screen pixels above the rendered body layer
        // (the 67-25 gap from the ACT data) and 1 pixel to the left.
        assert_eq!(delta, Vec2::new(-1.0, 42.0));
    }
}
