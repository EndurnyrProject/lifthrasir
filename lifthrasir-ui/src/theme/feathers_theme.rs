//! Norse `bevy_feathers` theme: the project palette mapped onto Feathers design
//! tokens so themed BSN nodes and Feathers controls resolve to one source of truth.
//!
//! Build the theme with [`norse_theme`] and install it via [`install_norse_theme`].
//! Window nodes reference the `TOKEN_*` constants below; Feathers controls keep their
//! own token names, remapped here onto the Endurnir colors.

use bevy::prelude::*;
use bevy_feathers::dark_theme::create_dark_theme;
use bevy_feathers::theme::{ThemeToken, UiTheme};
use bevy_feathers::tokens;

use super::{
    EMERALD, FIELD, GLASS, GLASS_2, GOLD, GOLD_FAINT, RARITY_COMMON, RARITY_FINE, RARITY_MAGIC,
    RARITY_RARE, STROKE, TEXT, TEXT_DIM, TEXT_FAINT,
};

/// Window / panel chrome tokens consumed by the equipment window's BSN nodes.
pub const TOKEN_WINDOW_BG: ThemeToken = ThemeToken::new_static("norse.window.bg");
pub const TOKEN_WINDOW_BORDER: ThemeToken = ThemeToken::new_static("norse.window.border");
pub const TOKEN_TITLEBAR_BG: ThemeToken = ThemeToken::new_static("norse.titlebar.bg");
pub const TOKEN_PANEL_BG: ThemeToken = ThemeToken::new_static("norse.panel.bg");
pub const TOKEN_PANEL_BORDER: ThemeToken = ThemeToken::new_static("norse.panel.border");
pub const TOKEN_ACCENT: ThemeToken = ThemeToken::new_static("norse.accent");

/// Text tiers.
pub const TOKEN_TEXT: ThemeToken = ThemeToken::new_static("norse.text");
pub const TOKEN_TEXT_DIM: ThemeToken = ThemeToken::new_static("norse.text.dim");
pub const TOKEN_TEXT_FAINT: ThemeToken = ThemeToken::new_static("norse.text.faint");

/// Item-rarity tints for slot borders, captions and tooltips.
pub const TOKEN_RARITY_COMMON: ThemeToken = ThemeToken::new_static("norse.rarity.common");
pub const TOKEN_RARITY_FINE: ThemeToken = ThemeToken::new_static("norse.rarity.fine");
pub const TOKEN_RARITY_MAGIC: ThemeToken = ThemeToken::new_static("norse.rarity.magic");
pub const TOKEN_RARITY_RARE: ThemeToken = ThemeToken::new_static("norse.rarity.rare");

/// Build the Norse [`UiTheme`]: start from the Feathers dark theme (so every control
/// token is populated) and remap the relevant tokens onto the Endurnir palette, then
/// add the Norse window and rarity tokens.
pub fn norse_theme() -> UiTheme {
    let mut theme = UiTheme(create_dark_theme());
    let color = &mut theme.0.color;

    color.insert(TOKEN_WINDOW_BG, GLASS);
    color.insert(TOKEN_WINDOW_BORDER, GOLD_FAINT);
    color.insert(TOKEN_TITLEBAR_BG, GLASS_2);
    color.insert(TOKEN_PANEL_BG, FIELD);
    color.insert(TOKEN_PANEL_BORDER, STROKE);
    color.insert(TOKEN_ACCENT, GOLD);
    color.insert(TOKEN_TEXT, TEXT);
    color.insert(TOKEN_TEXT_DIM, TEXT_DIM);
    color.insert(TOKEN_TEXT_FAINT, TEXT_FAINT);

    color.insert(TOKEN_RARITY_COMMON, RARITY_COMMON);
    color.insert(TOKEN_RARITY_FINE, RARITY_FINE);
    color.insert(TOKEN_RARITY_MAGIC, RARITY_MAGIC);
    color.insert(TOKEN_RARITY_RARE, RARITY_RARE);

    color.insert(tokens::WINDOW_BG, GLASS);
    color.insert(tokens::TEXT_MAIN, TEXT);
    color.insert(tokens::TEXT_DIM, TEXT_DIM);
    color.insert(tokens::FOCUS_RING, EMERALD);
    color.insert(tokens::BUTTON_BG, FIELD);
    color.insert(tokens::BUTTON_BG_HOVER, GLASS_2);
    color.insert(tokens::BUTTON_BG_PRESSED, EMERALD);
    color.insert(tokens::BUTTON_TEXT, TEXT);

    theme
}

/// Install the Norse theme as the active [`UiTheme`] resource.
pub fn install_norse_theme(app: &mut App) {
    app.insert_resource(norse_theme());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_tokens_resolve_to_palette() {
        let theme = norse_theme();
        assert_eq!(theme.color(&TOKEN_WINDOW_BG), GLASS);
        assert_eq!(theme.color(&TOKEN_WINDOW_BORDER), GOLD_FAINT);
        assert_eq!(theme.color(&tokens::WINDOW_BG), GLASS);
    }

    #[test]
    fn rarity_tokens_map_to_rarity_colors() {
        let theme = norse_theme();
        assert_eq!(theme.color(&TOKEN_RARITY_COMMON), RARITY_COMMON);
        assert_eq!(theme.color(&TOKEN_RARITY_FINE), RARITY_FINE);
        assert_eq!(theme.color(&TOKEN_RARITY_MAGIC), RARITY_MAGIC);
        assert_eq!(theme.color(&TOKEN_RARITY_RARE), RARITY_RARE);
    }
}
