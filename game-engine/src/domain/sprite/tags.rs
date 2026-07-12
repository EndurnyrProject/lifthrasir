use moonshine_tag::Tag;

use crate::domain::entities::character::components::equipment::EquipmentSlot;

pub const PIXELS_PER_METRE: f32 = 4.0;
pub const Z_OFFSET_PER_LAYER: f32 = 0.001;
pub const SPRITE_BASE_Y_OFFSET: f32 = -7.5;

moonshine_tag::tags! {
    pub LAYER_SHADOW,
    pub LAYER_BODY,
    pub LAYER_HEAD,
    pub LAYER_GARMENT,
    pub LAYER_WEAPON,
    pub LAYER_SHIELD,
    pub LAYER_HEAD_TOP,
    pub LAYER_HEAD_MID,
    pub LAYER_HEAD_BOTTOM,
    pub LAYER_EFFECT,
    pub LAYER_CART,
    pub FRAME_ATTACK,
    pub FRAME_SOUND,
}

pub fn layer_order(tag: Tag) -> u8 {
    match tag {
        t if t == LAYER_SHADOW => 0,
        t if t == LAYER_CART => 15,
        t if t == LAYER_HEAD => 10,
        t if t == LAYER_BODY => 20,
        t if t == LAYER_GARMENT => 30,
        t if t == LAYER_WEAPON => 40,
        t if t == LAYER_SHIELD => 50,
        t if t == LAYER_HEAD_BOTTOM => 60,
        t if t == LAYER_HEAD_MID => 70,
        t if t == LAYER_HEAD_TOP => 80,
        t if t == LAYER_EFFECT => 90,
        _ => 100,
    }
}

/// Transparent-sort depth bias for a layer's `StandardMaterial`.
///
/// A unit's billboard quads are near-coplanar, so the transparent pass
/// distance sort alone is undecided between them (the 0.001-scale z nudges
/// vanish in view-space distance) and Bevy's retained phase then falls back
/// to insertion order, which is arbitrary. The material `depth_bias` is added
/// to the sort distance (higher draws on top), deciding the in-unit stacking
/// deterministically. The step is far below inter-unit distances (cells are
/// 5 world units) and the effect solid tier (1.0), so it never reorders
/// across entities.
pub fn layer_depth_bias(tag: Tag) -> f32 {
    let rank = match tag {
        t if t == LAYER_SHADOW => 0,
        t if t == LAYER_CART => 1,
        t if t == LAYER_BODY => 2,
        t if t == LAYER_HEAD => 3,
        t if t == LAYER_GARMENT => 4,
        t if t == LAYER_WEAPON => 5,
        t if t == LAYER_SHIELD => 6,
        t if t == LAYER_HEAD_BOTTOM => 7,
        t if t == LAYER_HEAD_MID => 8,
        t if t == LAYER_HEAD_TOP => 9,
        t if t == LAYER_EFFECT => 10,
        _ => 11,
    };
    rank as f32 * 0.05
}

pub fn equipment_slot_to_tag(slot: &EquipmentSlot) -> Tag {
    match slot {
        EquipmentSlot::Weapon => LAYER_WEAPON,
        EquipmentSlot::Shield => LAYER_SHIELD,
        EquipmentSlot::Garment => LAYER_GARMENT,
        EquipmentSlot::HeadTop => LAYER_HEAD_TOP,
        EquipmentSlot::HeadMid => LAYER_HEAD_MID,
        EquipmentSlot::HeadBottom => LAYER_HEAD_BOTTOM,
        EquipmentSlot::Armor | EquipmentSlot::Shoes => LAYER_BODY,
        EquipmentSlot::Accessory1 | EquipmentSlot::Accessory2 => LAYER_EFFECT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cart_layer_orders_behind_body() {
        assert_eq!(layer_order(LAYER_CART), 15);
    }
}
