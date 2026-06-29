use crate::domain::entities::character::components::equipment::EquipmentSlot;

pub const EQP_HEAD_LOW: u32 = 0x000001;
pub const EQP_RIGHT_HAND: u32 = 0x000002;
pub const EQP_GARMENT: u32 = 0x000004;
pub const EQP_RIGHT_ACCESSORY: u32 = 0x000008;
pub const EQP_ARMOR: u32 = 0x000010;
pub const EQP_LEFT_HAND: u32 = 0x000020;
pub const EQP_SHOES: u32 = 0x000040;
pub const EQP_LEFT_ACCESSORY: u32 = 0x000080;
pub const EQP_HEAD_TOP: u32 = 0x000100;
pub const EQP_HEAD_MID: u32 = 0x000200;

const SLOT_BITS: [(u32, EquipmentSlot); 10] = [
    (EQP_HEAD_LOW, EquipmentSlot::HeadBottom),
    (EQP_RIGHT_HAND, EquipmentSlot::Weapon),
    (EQP_GARMENT, EquipmentSlot::Garment),
    (EQP_RIGHT_ACCESSORY, EquipmentSlot::Accessory1),
    (EQP_ARMOR, EquipmentSlot::Armor),
    (EQP_LEFT_HAND, EquipmentSlot::Shield),
    (EQP_SHOES, EquipmentSlot::Shoes),
    (EQP_LEFT_ACCESSORY, EquipmentSlot::Accessory2),
    (EQP_HEAD_TOP, EquipmentSlot::HeadTop),
    (EQP_HEAD_MID, EquipmentSlot::HeadMid),
];

pub fn decode_wear_location(mask: u32) -> Vec<EquipmentSlot> {
    SLOT_BITS
        .iter()
        .filter(|(bit, _)| mask & bit != 0)
        .map(|(_, slot)| *slot)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_top_decodes_to_single_slot() {
        assert_eq!(
            decode_wear_location(EQP_HEAD_TOP),
            vec![EquipmentSlot::HeadTop]
        );
    }

    #[test]
    fn head_mid_decodes_to_single_slot() {
        assert_eq!(
            decode_wear_location(EQP_HEAD_MID),
            vec![EquipmentSlot::HeadMid]
        );
    }

    #[test]
    fn head_low_decodes_to_single_slot() {
        assert_eq!(
            decode_wear_location(EQP_HEAD_LOW),
            vec![EquipmentSlot::HeadBottom]
        );
    }

    #[test]
    fn combined_mask_decodes_all_set_slots() {
        let mask = EQP_HEAD_LOW | EQP_HEAD_TOP | EQP_HEAD_MID;
        assert_eq!(
            decode_wear_location(mask),
            vec![
                EquipmentSlot::HeadBottom,
                EquipmentSlot::HeadTop,
                EquipmentSlot::HeadMid,
            ]
        );
    }

    #[test]
    fn zero_decodes_to_empty() {
        assert!(decode_wear_location(0).is_empty());
    }
}
