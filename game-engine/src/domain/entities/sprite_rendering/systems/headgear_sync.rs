use std::collections::HashMap;

use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

use crate::domain::entities::billboard::EquipmentPreviewCamera;
use crate::domain::entities::character::components::equipment::EquipmentSlot;
use crate::domain::entities::sprite_rendering::components::{
    HeadAttachPoint, HeadLayer, PlayerSprite, RenderLayer,
};
use crate::domain::entities::sprite_rendering::layout::{ActionLayout, PlayerLayout};
use crate::domain::entities::sprite_rendering::systems::head_sync::{
    head_billboard_delta, head_screen_offset,
};
use crate::domain::entities::sprite_rendering::systems::set_layer_texture;
use crate::domain::sprite::tags::{LAYER_HEAD, Z_OFFSET_PER_LAYER, layer_order};
use crate::domain::system_sets::SpriteRenderingSystems;
use crate::infrastructure::assets::ro_animation_asset::RoAnimationAsset;
use crate::utils::constants::SPRITE_WORLD_SCALE;

type HeadgearLayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static RenderLayer,
        &'static ChildOf,
        &'static MeshMaterial3d<StandardMaterial>,
        &'static mut Transform,
    ),
    Without<HeadLayer>,
>;

/// Query filter for the camera whose rotation orients the headgear billboard,
/// excluding render layers and the equipment-window preview camera.
type CameraFilter = (
    With<Camera3d>,
    Without<RenderLayer>,
    Without<EquipmentPreviewCamera>,
);

/// Per-frame snapshot the head publishes for headgear to align to.
struct HeadAnchor {
    attach_point: Vec2,
    frame_index: usize,
    layer_pos: Vec2,
    translation: Vec3,
}

fn is_headgear_slot(slot: EquipmentSlot) -> bool {
    matches!(
        slot,
        EquipmentSlot::HeadTop | EquipmentSlot::HeadMid | EquipmentSlot::HeadBottom
    )
}

/// Positions equipped headgear layers so they ride the head's per-frame anchor,
/// exactly the way `sync_player_head_layer` makes the head ride the body's anchor.
/// The head publishes its resolved attach point + frame via `HeadAttachPoint`; the
/// headgear reuses the same camera-rotated billboard-space delta math against it.
#[auto_add_system(
    plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::TransformUpdate, after = crate::domain::entities::sprite_rendering::systems::head_sync::sync_player_head_layer)
)]
pub fn sync_headgear_layer(
    animations: Res<Assets<RoAnimationAsset>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    camera_query: Query<&Transform, CameraFilter>,
    parent_query: Query<&PlayerSprite>,
    head_query: Query<(&HeadAttachPoint, &ChildOf, &Transform), With<HeadLayer>>,
    mut headgear_query: HeadgearLayerQuery,
) {
    let Ok(camera_transform) = camera_query.single() else {
        return;
    };

    let head_anchors: HashMap<Entity, HeadAnchor> = head_query
        .iter()
        .map(|(attach, child_of, transform)| {
            (
                child_of.parent(),
                HeadAnchor {
                    attach_point: attach.attach_point,
                    frame_index: attach.frame_index,
                    layer_pos: attach.layer_pos,
                    translation: transform.translation,
                },
            )
        })
        .collect();

    for (render_layer, child_of, material_handle, mut transform) in headgear_query.iter_mut() {
        let Some(slot) = render_layer.equipment_slot else {
            continue;
        };

        if !is_headgear_slot(slot) {
            continue;
        }

        let Some(head_anchor) = head_anchors.get(&child_of.parent()) else {
            continue;
        };

        let Ok(ro_sprite) = parent_query.get(child_of.parent()) else {
            continue;
        };

        let Some(animation) = animations.get(&render_layer.animation) else {
            continue;
        };

        let action_index =
            PlayerLayout::validate_action_index(ro_sprite.action_index(), animation.actions.len());
        let Some(action) = animation.actions.get(action_index) else {
            continue;
        };

        if action.frames.is_empty() {
            continue;
        }

        let frame_index = head_anchor
            .frame_index
            .min(action.frames.len().saturating_sub(1));
        let Some(frame) = action.frames.get(frame_index) else {
            continue;
        };

        let Some(part) = frame.parts.first() else {
            continue;
        };

        if let Some(texture) = animation.textures.get(part.texture_index) {
            set_layer_texture(&mut materials, &material_handle.0, texture);
        }

        let mut scale_x = part.scale.x * part.texture_size.x * SPRITE_WORLD_SCALE;
        let scale_y = part.scale.y * part.texture_size.y * SPRITE_WORLD_SCALE;

        if part.mirror {
            scale_x = -scale_x;
        }

        let new_scale = Vec3::new(scale_x, scale_y, 1.0);

        let Some(headgear_attach) = frame.attach_point else {
            let current = *transform;
            transform.set_if_neq(Transform {
                scale: new_scale,
                ..current
            });
            continue;
        };

        let screen_offset =
            head_screen_offset(part.position, head_anchor.attach_point, headgear_attach);
        let delta = head_billboard_delta(screen_offset, head_anchor.layer_pos) * SPRITE_WORLD_SCALE;
        let world_delta = camera_transform.rotation * delta.extend(0.0);

        // Stack the headgear in front of the head along the camera's view axis,
        // not world Z. Head and headgear sit at nearly the same point, so a world-Z
        // nudge barely changes camera depth under the tilted RO camera and the sort
        // is undecided. `world_delta` lies in the camera plane (zero depth), so a
        // push toward the camera is the only term that orders the two reliably.
        let layer_gap = (layer_order(render_layer.layer) as f32 - layer_order(LAYER_HEAD) as f32)
            * Z_OFFSET_PER_LAYER;
        let towards_camera = -camera_transform.forward().as_vec3();

        let current = *transform;
        transform.set_if_neq(Transform {
            scale: new_scale,
            translation: head_anchor.translation + world_delta + towards_camera * layer_gap,
            ..current
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Stored coordinates (Y negated during extraction, so Y is up). The head's
    // published attach data plays the role the body plays for the head.
    const HEAD_LAYER_POS: Vec2 = Vec2::new(-1.0, 67.0);
    const HEAD_ATTACH: Vec2 = Vec2::new(2.0, 70.0);
    const HEADGEAR_LAYER_POS: Vec2 = Vec2::new(0.0, 80.0);
    const HEADGEAR_ATTACH: Vec2 = Vec2::new(2.0, 70.0);

    #[test]
    fn headgear_screen_offset_aligns_to_head_anchor() {
        let offset = head_screen_offset(HEADGEAR_LAYER_POS, HEAD_ATTACH, HEADGEAR_ATTACH);
        let delta = head_billboard_delta(offset, HEAD_LAYER_POS);

        // Attach points match, so the headgear sits at its own layer position; the
        // delta is purely the gap between that and the head's layer position.
        assert_eq!(offset, HEADGEAR_LAYER_POS);
        assert_eq!(delta, Vec2::new(1.0, 13.0));
    }

    #[test]
    fn headgear_screen_offset_identity_when_attach_points_match() {
        let headgear_layer_pos = Vec2::new(4.0, 9.0);
        let attach = Vec2::new(-3.0, 12.0);

        assert_eq!(
            head_screen_offset(headgear_layer_pos, attach, attach),
            headgear_layer_pos
        );
    }

    #[test]
    fn headgear_screen_offset_shifts_when_attach_points_differ() {
        let offset = head_screen_offset(HEADGEAR_LAYER_POS, HEAD_ATTACH, Vec2::new(5.0, 66.0));

        // head_attach - headgear_attach = (-3, 4) applied to the layer position.
        assert_eq!(offset, Vec2::new(-3.0, 84.0));
    }
}
