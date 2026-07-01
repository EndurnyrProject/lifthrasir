use std::collections::HashMap;

use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;

use crate::domain::entities::billboard::EquipmentPreviewCamera;
use crate::domain::entities::character::components::equipment::EquipmentSlot;
use crate::domain::entities::sprite_rendering::components::{
    BodyAttachPoint, HeadLayer, PlayerSprite, RenderLayer,
};
use crate::domain::entities::sprite_rendering::layout::{ActionLayout, PlayerLayout};
use crate::domain::entities::sprite_rendering::systems::head_sync::{
    head_billboard_delta, head_screen_offset,
};
use crate::domain::sprite::tags::{layer_order, LAYER_BODY, Z_OFFSET_PER_LAYER};
use crate::domain::system_sets::SpriteRenderingSystems;
use crate::infrastructure::assets::ro_animation_asset::RoAnimationAsset;
use crate::utils::constants::SPRITE_WORLD_SCALE;

type WeaponLayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static RenderLayer,
        &'static ChildOf,
        &'static MeshMaterial3d<StandardMaterial>,
        &'static mut Transform,
        &'static mut Visibility,
    ),
    // `Without<BodyAttachPoint>` keeps this mutable-`Transform` query disjoint
    // from `body_query`'s immutable `&Transform` (the body layer carries
    // `BodyAttachPoint`), avoiding a B0001 query-conflict panic at plugin init.
    (Without<HeadLayer>, Without<BodyAttachPoint>),
>;

/// Query filter for the camera whose rotation orients the weapon billboard,
/// excluding render layers and the equipment-window preview camera.
type CameraFilter = (
    With<Camera3d>,
    Without<RenderLayer>,
    Without<EquipmentPreviewCamera>,
);

/// Per-frame snapshot the body publishes for weapon/shield layers to align to.
struct BodyAnchor {
    attach_point: Vec2,
    frame_index: usize,
    layer_pos: Vec2,
    translation: Vec3,
}

fn is_weapon_slot(slot: EquipmentSlot) -> bool {
    matches!(slot, EquipmentSlot::Weapon | EquipmentSlot::Shield)
}

/// Positions equipped weapon and shield layers so they ride the body's per-frame
/// anchor, exactly the way `sync_player_head_layer` makes the head ride the body.
/// The body publishes its resolved attach point + frame via `BodyAttachPoint`; the
/// weapon/shield reuse the same camera-rotated billboard-space delta math against
/// it (not the head anchor headgear uses).
#[auto_add_system(
    plugin = crate::app::sprite_rendering_domain_plugin::SpriteRenderingDomainPlugin,
    schedule = Update,
    config(in_set = SpriteRenderingSystems::TransformUpdate, after = crate::domain::entities::sprite_rendering::systems::body_sync::sync_player_body_layer)
)]
pub fn sync_weapon_layer(
    animations: Res<Assets<RoAnimationAsset>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    camera_query: Query<&Transform, CameraFilter>,
    parent_query: Query<&PlayerSprite>,
    body_query: Query<(&BodyAttachPoint, &ChildOf, &Transform), Without<HeadLayer>>,
    mut weapon_query: WeaponLayerQuery,
) {
    let Ok(camera_transform) = camera_query.single() else {
        return;
    };

    let body_anchors: HashMap<Entity, BodyAnchor> = body_query
        .iter()
        .map(|(attach, child_of, transform)| {
            (
                child_of.parent(),
                BodyAnchor {
                    attach_point: attach.attach_point,
                    frame_index: attach.frame_index,
                    layer_pos: attach.layer_pos,
                    translation: transform.translation,
                },
            )
        })
        .collect();

    for (render_layer, child_of, material_handle, mut transform, mut visibility) in
        weapon_query.iter_mut()
    {
        let Some(slot) = render_layer.equipment_slot else {
            continue;
        };

        if !is_weapon_slot(slot) {
            continue;
        }

        let Some(body_anchor) = body_anchors.get(&child_of.parent()) else {
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

        // The weapon ACT only carries frames for the stance/attack actions; for
        // every other action (idle, walk, ...) the frame has no parts. Hide the
        // layer then, so the last drawn sprite doesn't linger floating in place.
        let Some(frame) = animation.actions.get(action_index).and_then(|action| {
            let frame_index = body_anchor
                .frame_index
                .min(action.frames.len().saturating_sub(1));
            action.frames.get(frame_index)
        }) else {
            *visibility = Visibility::Hidden;
            continue;
        };

        let Some(part) = frame.parts.first() else {
            *visibility = Visibility::Hidden;
            continue;
        };

        *visibility = Visibility::Inherited;

        if let Some(texture) = animation.textures.get(part.texture_index) {
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

        // Weapon/shield frames usually carry no anchor of their own; the
        // reference client then places them by their raw layer position relative
        // to the body (no attach correction). When an anchor is present, align it
        // to the body's attach point like the head does.
        let screen_offset = match frame.attach_point {
            Some(weapon_attach) => {
                head_screen_offset(part.position, body_anchor.attach_point, weapon_attach)
            }
            None => part.position,
        };
        let delta = head_billboard_delta(screen_offset, body_anchor.layer_pos) * SPRITE_WORLD_SCALE;
        let world_delta = camera_transform.rotation * delta.extend(0.0);

        let layer_gap = (layer_order(render_layer.layer) as f32 - layer_order(LAYER_BODY) as f32)
            * Z_OFFSET_PER_LAYER;

        transform.translation =
            body_anchor.translation + world_delta + Vec3::new(0.0, 0.0, layer_gap);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Stored coordinates (Y negated during extraction, so Y is up). The body's
    // published attach data plays the same role for the weapon as it does for the
    // head, so the offset math is the head's math reused verbatim.
    const BODY_LAYER_POS: Vec2 = Vec2::new(0.0, 25.0);
    const BODY_ATTACH: Vec2 = Vec2::new(1.0, 56.0);
    const WEAPON_LAYER_POS: Vec2 = Vec2::new(-8.0, 40.0);
    const WEAPON_ATTACH: Vec2 = Vec2::new(1.0, 56.0);

    // Building the schedule with `sync_weapon_layer` panics with B0001 if its
    // `Transform` queries alias. The pure-helper tests never build the schedule,
    // so this guards the disjointness `WeaponLayerQuery`'s filter relies on.
    #[test]
    fn sync_weapon_layer_has_no_query_conflict() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()));
        app.init_asset::<RoAnimationAsset>();
        app.init_asset::<StandardMaterial>();
        app.add_systems(Update, sync_weapon_layer);
        app.update();
    }

    #[test]
    fn is_weapon_slot_matches_weapon_and_shield() {
        assert!(is_weapon_slot(EquipmentSlot::Weapon));
        assert!(is_weapon_slot(EquipmentSlot::Shield));
        assert!(!is_weapon_slot(EquipmentSlot::HeadTop));
        assert!(!is_weapon_slot(EquipmentSlot::Armor));
    }

    #[test]
    fn weapon_screen_offset_aligns_to_body_anchor() {
        let offset = head_screen_offset(WEAPON_LAYER_POS, BODY_ATTACH, WEAPON_ATTACH);
        let delta = head_billboard_delta(offset, BODY_LAYER_POS);

        // Attach points match, so the weapon sits at its own layer position; the
        // delta is purely the gap between that and the body's layer position.
        assert_eq!(offset, WEAPON_LAYER_POS);
        assert_eq!(delta, Vec2::new(-8.0, 15.0));
    }

    #[test]
    fn weapon_screen_offset_shifts_when_attach_points_differ() {
        let offset = head_screen_offset(WEAPON_LAYER_POS, BODY_ATTACH, Vec2::new(4.0, 60.0));

        // body_attach - weapon_attach = (-3, -4) applied to the layer position.
        assert_eq!(offset, Vec2::new(-11.0, 36.0));
    }
}
