use bevy::prelude::*;

use crate::theme;

pub const BEAM: &str = "ro://ui/beam.svg";
pub const RING: &str = "ro://ui/ring.svg";
pub const RING_THIN: &str = "ro://ui/ring-thin.svg";
pub const VACANT_PAD: &str = "ro://ui/vacant-pad.svg";
pub const GRAIN: &str = "ro://ui/grain.svg";
pub const SCENIC_TEXT_SHADOW_ALPHA: f32 = 0.85;

pub fn scenic_text_shadow() -> TextShadow {
    TextShadow {
        offset: Vec2::splat(2.0),
        color: Color::BLACK.with_alpha(SCENIC_TEXT_SHADOW_ALPHA),
    }
}

pub fn class_hue(class_id: u16) -> Color {
    const SWORD: Color = Color::srgb_u8(0xd9, 0xa4, 0x41);
    const MAGE: Color = Color::srgb_u8(0x6f, 0xc4, 0xec);
    const ARCHER: Color = Color::srgb_u8(0x78, 0xc9, 0x67);
    const ACOLYTE: Color = Color::srgb_u8(0x4f, 0xc7, 0xaa);
    const MERCHANT: Color = Color::srgb_u8(0xe6, 0xb5, 0x52);
    const THIEF: Color = Color::srgb_u8(0xb2, 0x7b, 0xd9);

    match class_id {
        0 | 4001 => theme::EMERALD,
        1 | 7 | 14 | 4002 | 4008 | 4015 => SWORD,
        2 | 9 | 16 | 4003 | 4010 | 4017 => MAGE,
        3 | 11 | 19 | 20 | 4004 | 4012 | 4020 | 4021 => ARCHER,
        4 | 8 | 15 | 4005 | 4009 | 4016 => ACOLYTE,
        5 | 10 | 18 | 4006 | 4011 | 4019 => MERCHANT,
        6 | 12 | 17 | 4007 | 4013 | 4018 => THIEF,
        _ => theme::EMERALD,
    }
}

pub fn job_level_cap(class_id: u16) -> u32 {
    if matches!(class_id, 0 | 4001) { 10 } else { 50 }
}

pub fn mono_label(text: &str) -> String {
    let mut label = String::new();
    for glyph in text.to_uppercase().chars() {
        if !label.is_empty() {
            label.push('\u{200a}');
        }
        label.push(glyph);
    }
    label
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_classes_have_distinct_non_fallback_hues() {
        let hues = [
            class_hue(1),
            class_hue(2),
            class_hue(3),
            class_hue(4),
            class_hue(5),
            class_hue(6),
        ];

        assert!(hues.iter().all(|&hue| hue != theme::EMERALD));
        for (index, hue) in hues.iter().enumerate() {
            assert!(hues[index + 1..].iter().all(|other| hue != other));
        }
        assert_eq!(class_hue(7), class_hue(1));
        assert_eq!(class_hue(4008), class_hue(1));
        assert_eq!(class_hue(0), theme::EMERALD);
        assert_eq!(class_hue(u16::MAX), theme::EMERALD);
    }

    #[test]
    fn job_level_caps_cover_novice_and_job_tiers() {
        assert_eq!(job_level_cap(0), 10);
        assert_eq!(job_level_cap(1), 50);
        assert_eq!(job_level_cap(7), 50);
        assert_eq!(job_level_cap(4008), 50);
        assert_eq!(job_level_cap(u16::MAX), 50);
    }

    #[test]
    fn mono_labels_are_uppercase_and_hair_spaced() {
        assert_eq!(
            mono_label("Endurnir"),
            "E\u{200a}N\u{200a}D\u{200a}U\u{200a}R\u{200a}N\u{200a}I\u{200a}R"
        );
    }
}
