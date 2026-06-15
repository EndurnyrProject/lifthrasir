use bevy::prelude::*;

pub const FORGE_SOOT: Color = Color::srgb_u8(0x1A, 0x1A, 0x1A);
pub const SLATE_GRAY: Color = Color::srgb_u8(0x2D, 0x30, 0x38);
pub const POLISHED_STEEL: Color = Color::srgb_u8(0x44, 0x44, 0x44);
pub const ENERGETIC_GREEN: Color = Color::srgb_u8(0x00, 0xE5, 0x7A);
pub const ASHEN_WHITE: Color = Color::srgb_u8(0xE1, 0xE1, 0xE1);
pub const WORN_CRIMSON: Color = Color::srgb_u8(0xA4, 0x42, 0x42);
pub const HEALTH_RED: Color = Color::srgb_u8(0xE7, 0x4C, 0x3C);
pub const MANA_BLUE: Color = Color::srgb_u8(0x34, 0x98, 0xDB);
pub const GOLD_YELLOW: Color = Color::srgb_u8(0xF1, 0xC4, 0x0F);

pub const FONT_TITLE: &str = "fonts/ringbearer-medium.ttf";
pub const FONT_BODY: &str = "fonts/palatino-linotype-regular.ttf";
pub const FONT_BODY_BOLD: &str = "fonts/palatino-linotype-bold.ttf";

pub fn panel_node() -> impl Bundle {
    (
        Node {
            padding: UiRect::all(Val::Px(16.0)),
            border: UiRect::all(Val::Px(1.0)),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        BackgroundColor(SLATE_GRAY),
        BorderColor::all(POLISHED_STEEL),
    )
}
