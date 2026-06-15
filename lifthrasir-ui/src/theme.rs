use bevy::prelude::*;

// Endurnir palette — the single source of truth for UI colors (raw bevy_ui).
pub const GLASS: Color = Color::srgba(0.063, 0.086, 0.078, 0.97);
pub const GLASS_2: Color = Color::srgba(0.086, 0.118, 0.106, 0.97);
pub const FIELD: Color = Color::srgba(0.031, 0.047, 0.043, 0.66);
pub const EMERALD: Color = Color::srgb_u8(0x2f, 0xd2, 0x7a);
pub const EMERALD_BRI: Color = Color::srgb_u8(0x46, 0xe0, 0x8c);
pub const EMERALD_DEEP: Color = Color::srgb_u8(0x1e, 0xa8, 0x62);
pub const EMERALD_INK: Color = Color::srgb_u8(0x06, 0x35, 0x1f);
pub const GOLD: Color = Color::srgb_u8(0xd9, 0xa4, 0x41);
pub const GOLD_FAINT: Color = Color::srgba(0.851, 0.643, 0.255, 0.22);
pub const STROKE: Color = Color::srgba(1.0, 1.0, 1.0, 0.085);
pub const STROKE_STRONG: Color = Color::srgba(1.0, 1.0, 1.0, 0.16);
pub const TEXT: Color = Color::srgb_u8(0xea, 0xf1, 0xea);
pub const TEXT_DIM: Color = Color::srgb_u8(0x9f, 0xb1, 0xa6);
pub const TEXT_FAINT: Color = Color::srgb_u8(0x6f, 0x80, 0x77);
pub const DISPLAY_GOLD: Color = Color::srgb_u8(0xf1, 0xea, 0xd9);
pub const HEALTH_RED: Color = Color::srgb_u8(0xe7, 0x4c, 0x3c);
pub const MANA_BLUE: Color = Color::srgb_u8(0x4f, 0xb6, 0xe6);
pub const BAD: Color = Color::srgb_u8(0xe0, 0x62, 0x5e);
pub const WARN: Color = Color::srgb_u8(0xe6, 0xb5, 0x52);

pub const FONT_TITLE: &str = "fonts/cinzel.ttf";
pub const FONT_BODY: &str = "fonts/manrope.ttf";
pub const FONT_BODY_BOLD: &str = "fonts/manrope.ttf";

/// Glass panel: translucent dark fill, gold-faint hairline border, rounded.
pub fn glass_panel() -> impl Bundle {
    (
        Node {
            padding: UiRect::all(Val::Px(16.0)),
            border: UiRect::all(Val::Px(1.0)),
            flex_direction: FlexDirection::Column,
            border_radius: BorderRadius::all(Val::Px(11.0)),
            ..default()
        },
        BackgroundColor(GLASS),
        BorderColor::all(GOLD_FAINT),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emerald_is_brand_hex() {
        // #2fd27a — the Endurnir emerald.
        assert_eq!(EMERALD, Color::srgb_u8(0x2f, 0xd2, 0x7a));
    }

    #[test]
    fn fonts_point_at_vendored_ttf() {
        assert_eq!(FONT_BODY, "fonts/manrope.ttf");
        assert_eq!(FONT_TITLE, "fonts/cinzel.ttf");
    }
}
