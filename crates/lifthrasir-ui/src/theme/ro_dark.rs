//! RO-dark `bevy_feathers` theme: slate panels, bronze trim, frost (primary/focus)
//! and oxblood (danger) accents, mapped onto Feathers design tokens. Successor of
//! [`super::feathers_theme`]; the art it pairs with lives in
//! [`crate::widgets::primitives`].
//!
//! The palette is copied from the design source (`lifthrasir-ui-ro/primitives/README.md`,
//! "Palette"). New screens take colours from this list instead of inventing new ones.
//!
//! NOTE: the host still inserts `norse_theme`; swap to [`ro_dark_theme`] once no screen
//! reads the `norse.*` tokens, or those nodes render fuchsia.

use bevy::prelude::Color;
use bevy_feathers::dark_theme::create_dark_theme;
use bevy_feathers::theme::{ThemeToken, UiTheme};
use bevy_feathers::tokens;

use super::feathers_theme::assign_token;

// Surfaces.
pub const CANVAS: Color = Color::srgb_u8(0x15, 0x17, 0x1b);
pub const WINDOW_BODY: Color = Color::srgb_u8(0x31, 0x38, 0x44);
pub const TITLE_BAR: Color = Color::srgb_u8(0x3f, 0x48, 0x55);
pub const TOOLTIP_BODY: Color = Color::srgb_u8(0x2b, 0x31, 0x3b);
pub const INSET: Color = Color::srgb_u8(0x1b, 0x1f, 0x25);
pub const CORNER_DISC: Color = Color::srgb_u8(0x1c, 0x20, 0x27);
pub const BAR_TROUGH: Color = Color::srgb_u8(0x11, 0x13, 0x17);
pub const SCROLL_TRACK: Color = Color::srgb_u8(0x15, 0x18, 0x1d);

// Tabs and slots.
pub const TAB_ACTIVE: Color = Color::srgb_u8(0x3c, 0x45, 0x52);
pub const TAB_HOVER: Color = Color::srgb_u8(0x30, 0x36, 0x3f);
pub const TAB_INACTIVE: Color = Color::srgb_u8(0x27, 0x2c, 0x34);
pub const SLOT: Color = Color::srgb_u8(0x22, 0x27, 0x2e);
pub const SLOT_HOVER: Color = Color::srgb_u8(0x2a, 0x30, 0x38);
pub const SLOT_DISABLED: Color = Color::srgb_u8(0x1a, 0x1d, 0x22);
pub const SLOT_STROKE: Color = Color::srgb_u8(0x3a, 0x41, 0x4c);
pub const WINDOW_OUTLINE: Color = Color::srgb_u8(0x09, 0x0b, 0x0e);

// Buttons.
pub const BUTTON: Color = Color::srgb_u8(0x3d, 0x46, 0x54);
pub const BUTTON_HOVER: Color = Color::srgb_u8(0x4b, 0x57, 0x68);
pub const BUTTON_PRESSED: Color = Color::srgb_u8(0x2f, 0x36, 0x41);
pub const BUTTON_DISABLED: Color = Color::srgb_u8(0x27, 0x2b, 0x31);
pub const PRIMARY: Color = Color::srgb_u8(0x2f, 0x5f, 0x75);
pub const PRIMARY_HOVER: Color = Color::srgb_u8(0x3a, 0x71, 0x89);
pub const PRIMARY_PRESSED: Color = Color::srgb_u8(0x24, 0x4a, 0x5c);
pub const DISABLED_STROKE: Color = Color::srgb_u8(0x35, 0x3a, 0x43);

// Danger.
pub const DANGER: Color = Color::srgb_u8(0x6a, 0x2b, 0x26);
pub const DANGER_HOVER: Color = Color::srgb_u8(0x7d, 0x34, 0x2e);
pub const DANGER_PRESSED: Color = Color::srgb_u8(0x52, 0x21, 0x1d);
pub const CLOSE: Color = Color::srgb_u8(0x5a, 0x2c, 0x27);
pub const CLOSE_HOVER: Color = Color::srgb_u8(0x7a, 0x3a, 0x33);
pub const DANGER_GLOW: Color = Color::srgb_u8(0xff, 0x9a, 0x7a);

// Bronze trim.
pub const BRONZE_LIGHT: Color = Color::srgb_u8(0xe7, 0xc9, 0x8f);
pub const PRIMARY_STROKE_HOVER: Color = Color::srgb_u8(0xe0, 0xbf, 0x80);
pub const BRONZE_HIGHLIGHT: Color = Color::srgb_u8(0xd2, 0xb0, 0x75);
/// Slider knob and primary button stroke.
pub const PRIMARY_STROKE: Color = Color::srgb_u8(0xc9, 0xa7, 0x6a);
pub const BRONZE: Color = Color::srgb_u8(0xb0, 0x8c, 0x55);
pub const DIVIDER: Color = Color::srgb_u8(0x9c, 0x7f, 0x50);
pub const TAB_STROKE: Color = Color::srgb_u8(0x9c, 0x7c, 0x4a);
pub const BUTTON_STROKE: Color = Color::srgb_u8(0x8c, 0x71, 0x46);
pub const BRONZE_SHADOW: Color = Color::srgb_u8(0x6e, 0x55, 0x32);
pub const BAR_FRAME: Color = Color::srgb_u8(0x6e, 0x5c, 0x40);
pub const MUTED_BRONZE: Color = Color::srgb_u8(0x6b, 0x5a, 0x40);

// Frost accent.
pub const CHECK_MARK: Color = Color::srgb_u8(0xb5, 0xe3, 0xf5);
pub const KNOB_HOVER: Color = Color::srgb_u8(0xa9, 0xdb, 0xf0);
pub const SELECT_RING: Color = Color::srgb_u8(0x9f, 0xdc, 0xff);
/// Also the focus ring.
pub const FROST_GLOW: Color = Color::srgb_u8(0x8f, 0xd0, 0xff);
pub const FROST: Color = Color::srgb_u8(0x8c, 0xc6, 0xe0);

// Rare.
pub const RARE_RING: Color = Color::srgb_u8(0xe2, 0xb6, 0x5e);
pub const RARE_GLOW: Color = Color::srgb_u8(0xf0, 0xc0, 0x60);

// Stat bars.
pub const HP: Color = Color::srgb_u8(0xb6, 0x46, 0x3d);
pub const SP: Color = Color::srgb_u8(0x3f, 0x78, 0xb4);
pub const BASE_EXP: Color = Color::srgb_u8(0xc9, 0x9a, 0x45);
pub const JOB_EXP: Color = Color::srgb_u8(0x6f, 0x9a, 0x5c);

// Server status.
pub const STATUS_ONLINE: Color = Color::srgb_u8(0x8f, 0xbf, 0x7a);
pub const STATUS_BUSY: Color = Color::srgb_u8(0xd9, 0xb2, 0x65);
pub const STATUS_FULL: Color = Color::srgb_u8(0xd0, 0x67, 0x5c);
pub const STATUS_OFFLINE: Color = Color::srgb_u8(0x7d, 0x84, 0x8e);

/// Hairline around rows and troughs inside an [`INSET`] panel.
pub const INSET_RULE: Color = Color::srgb_u8(0x2c, 0x32, 0x3b);

// Text.
pub const HERO: Color = Color::srgb_u8(0xf0, 0xe4, 0xc4);
pub const STRONG: Color = Color::srgb_u8(0xf4, 0xf6, 0xf8);
pub const FAINT: Color = Color::srgb_u8(0x80, 0x87, 0x8f);
pub const TITLE: Color = Color::srgb_u8(0xe8, 0xd8, 0xb0);
pub const VALUE: Color = Color::srgb_u8(0xf1, 0xed, 0xe4);
pub const BUTTON_LABEL: Color = Color::srgb_u8(0xe9, 0xe2, 0xd0);
pub const CLOSE_X: Color = Color::srgb_u8(0xe6, 0xdc, 0xcb);
pub const BODY: Color = Color::srgb_u8(0xdc, 0xdf, 0xe4);
pub const LABEL: Color = Color::srgb_u8(0xc8, 0xcc, 0xd2);
pub const MUTED: Color = Color::srgb_u8(0x9a, 0xa1, 0xaa);
pub const DISABLED_LABEL: Color = Color::srgb_u8(0x8b, 0x8f, 0x96);
pub const ACCENT: Color = Color::srgb_u8(0xb9, 0xa5, 0x7a);
pub const NPC_NAME: Color = Color::srgb_u8(0xd9, 0xb8, 0x74);

/// Window / panel chrome tokens for flat (non-art) BSN nodes.
pub const TOKEN_WINDOW_BG: ThemeToken = ThemeToken::new_static("ro_dark.window.bg");
pub const TOKEN_WINDOW_BORDER: ThemeToken = ThemeToken::new_static("ro_dark.window.border");
pub const TOKEN_TITLEBAR_BG: ThemeToken = ThemeToken::new_static("ro_dark.titlebar.bg");
pub const TOKEN_PANEL_BG: ThemeToken = ThemeToken::new_static("ro_dark.panel.bg");
pub const TOKEN_PANEL_BORDER: ThemeToken = ThemeToken::new_static("ro_dark.panel.border");
pub const TOKEN_ACCENT: ThemeToken = ThemeToken::new_static("ro_dark.accent");

/// Text tiers.
pub const TOKEN_TITLE: ThemeToken = ThemeToken::new_static("ro_dark.text.title");
pub const TOKEN_TEXT: ThemeToken = ThemeToken::new_static("ro_dark.text");
pub const TOKEN_TEXT_DIM: ThemeToken = ThemeToken::new_static("ro_dark.text.dim");
pub const TOKEN_TEXT_FAINT: ThemeToken = ThemeToken::new_static("ro_dark.text.faint");

/// Build the RO-dark [`UiTheme`]: start from the Feathers dark theme (so every control
/// token is populated), add the RO-dark tokens and remap the Feathers controls onto the
/// palette.
pub fn ro_dark_theme() -> UiTheme {
    let mut theme = create_dark_theme();

    assign_token(&mut theme, TOKEN_WINDOW_BG, WINDOW_BODY);
    assign_token(&mut theme, TOKEN_WINDOW_BORDER, WINDOW_OUTLINE);
    assign_token(&mut theme, TOKEN_TITLEBAR_BG, TITLE_BAR);
    assign_token(&mut theme, TOKEN_PANEL_BG, INSET);
    assign_token(&mut theme, TOKEN_PANEL_BORDER, SLOT_STROKE);
    assign_token(&mut theme, TOKEN_ACCENT, BRONZE);
    assign_token(&mut theme, TOKEN_TITLE, TITLE);
    assign_token(&mut theme, TOKEN_TEXT, BODY);
    assign_token(&mut theme, TOKEN_TEXT_DIM, MUTED);
    assign_token(&mut theme, TOKEN_TEXT_FAINT, DISABLED_LABEL);

    assign_token(&mut theme, tokens::WINDOW_BG, WINDOW_BODY);
    assign_token(&mut theme, tokens::TEXT_MAIN, BODY);
    assign_token(&mut theme, tokens::TEXT_DIM, MUTED);
    assign_token(&mut theme, tokens::FOCUS_RING, FROST_GLOW);
    assign_token(&mut theme, tokens::BUTTON_BG, BUTTON);
    assign_token(&mut theme, tokens::BUTTON_BG_HOVER, BUTTON_HOVER);
    assign_token(&mut theme, tokens::BUTTON_BG_PRESSED, BUTTON_PRESSED);
    assign_token(&mut theme, tokens::BUTTON_BG_DISABLED, BUTTON_DISABLED);
    assign_token(&mut theme, tokens::BUTTON_TEXT, BUTTON_LABEL);
    assign_token(&mut theme, tokens::BUTTON_TEXT_DISABLED, DISABLED_LABEL);
    assign_token(&mut theme, tokens::BUTTON_PRIMARY_BG, PRIMARY);
    assign_token(&mut theme, tokens::BUTTON_PRIMARY_BG_HOVER, PRIMARY_HOVER);
    assign_token(
        &mut theme,
        tokens::BUTTON_PRIMARY_BG_PRESSED,
        PRIMARY_PRESSED,
    );
    assign_token(&mut theme, tokens::BUTTON_PRIMARY_TEXT, BUTTON_LABEL);

    UiTheme(theme)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_resolve_to_palette() {
        let theme = ro_dark_theme();
        assert_eq!(theme.color(&TOKEN_WINDOW_BG), WINDOW_BODY);
        assert_eq!(theme.color(&TOKEN_TITLE), TITLE);
        assert_eq!(theme.color(&tokens::FOCUS_RING), FROST_GLOW);
        assert_eq!(theme.color(&tokens::BUTTON_PRIMARY_BG), PRIMARY);
    }
}
