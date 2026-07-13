use bevy::prelude::*;

pub mod feathers_theme;

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

// Floating combat numbers: yellow for damage the player deals, red for damage it takes.
pub const DAMAGE_DEALT: Color = Color::srgb_u8(0xf2, 0xd6, 0x4b);
pub const DAMAGE_RECEIVED: Color = HEALTH_RED;

// Item-rarity tints (from the Endurnir mockups): common reuses TEXT, fine the bright
// emerald, rare the gold, magic a cold blue. These feed the rarity theme tokens.
pub const RARITY_COMMON: Color = TEXT;
pub const RARITY_FINE: Color = EMERALD_BRI;
pub const RARITY_RARE: Color = GOLD;
pub const RARITY_MAGIC: Color = Color::srgb_u8(0x6f, 0xc4, 0xec);

pub const FONT_TITLE: &str = "fonts/cinzel.ttf";
pub const FONT_BODY: &str = "fonts/manrope.ttf";

/// Asset directory for the SVG glyph icons extracted from the Endurnir mockups.
pub const ICON_DIR: &str = "ui/icons/";

/// A square UI icon loaded from `assets/ui/icons/<name>.svg`. The icons ship as white
/// glyphs, so `color` is what sets their final tint (emerald play, crimson trash, …).
/// `Pickable::IGNORE` keeps the glyph from swallowing clicks on its host button.
pub fn icon(assets: &AssetServer, name: &str, size: f32, color: Color) -> impl Bundle {
    (
        ImageNode {
            image: assets.load(format!("{ICON_DIR}{name}.svg")),
            color,
            ..default()
        },
        Node {
            width: Val::Px(size),
            height: Val::Px(size),
            ..default()
        },
        Pickable::IGNORE,
    )
}

/// A plain text label. `Pickable::IGNORE` keeps it from blocking hover/clicks on the
/// row that hosts it. Pass `""` for labels whose text is filled in by a later system.
pub fn label(text: impl Into<String>, font: Handle<Font>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font: font.into(),
            font_size: size.into(),
            ..default()
        },
        TextColor(color),
        Pickable::IGNORE,
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
