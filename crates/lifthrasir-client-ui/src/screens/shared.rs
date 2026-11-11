//! Helpers any menu screen can use: number formatting, header pieces and the layout
//! and state helpers for the RO-dark art buttons.

use bevy::prelude::*;
use bevy::text::{FontSize, FontSourceTemplate};
use lifthrasir_ui::theme::{self, ro_dark};
use lifthrasir_ui::widgets::chrome::chrome_text;
use lifthrasir_ui::widgets::primitives::{BUTTON_SHADOW, ButtonArt, button};

/// Art tint of a button that cannot be used.
pub const DISABLED_TINT: Color = Color::srgb(0.45, 0.45, 0.45);
/// NOTE: the primitives ship no danger button art, so destructive buttons tint the
/// normal art. Swap for a `button_danger*.png` set once it exists.
pub const DANGER_TINT: Color = Color::srgb(1.0, 0.56, 0.52);

/// `n` with `,` between thousands groups.
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// A rounded tag naming the world (and slot) a screen acts on.
pub fn pill(label: String) -> impl Scene {
    bsn! {
        Node {
            height: px(24),
            align_items: AlignItems::Center,
            column_gap: px(6),
            padding: {UiRect::horizontal(px(10))},
            border: px(1),
            border_radius: BorderRadius::all(px(12)),
        }
        BackgroundColor({ro_dark::INSET.with_alpha(0.85)})
        BorderColor::all(ro_dark::MUTED_BRONZE)
        Children [
            Node { width: px(6), height: px(6), border_radius: BorderRadius::MAX }
            BackgroundColor(ro_dark::STATUS_BUSY)
            --
            @chrome_text(label, 12.0, ro_dark::STRONG)
        ]
    }
}

/// Frost text that acts as a link. The caller attaches the click observer.
pub fn link(label: &'static str) -> impl Scene {
    bsn! {
        Text(label)
        TextFont {
            font: FontSourceTemplate::Handle(theme::FONT_BODY),
            font_size: {FontSize::Px(12.0)},
        }
        TextColor(ro_dark::FROST)
        TextShadow { offset: Vec2::splat(1.0), color: {Color::BLACK.with_alpha(0.9)} }
        Pickable
    }
}

/// An art [`button`] laid out by its shape: the negative margin cancels the baked
/// shadow, so rows and gaps follow the design sizes. Widths set by the caller still
/// include `2 * BUTTON_SHADOW`.
pub fn flush_button(art: ButtonArt, label: &'static str) -> impl Scene {
    bsn! {
        @button(art, label)
        Node { margin: {UiRect::all(px(-BUTTON_SHADOW))} }
    }
}

/// Tint and picking of an art button that may be unavailable: a disabled one is greyed
/// out and neither hovers nor activates.
pub fn usable(enabled: bool, tint: Color) -> impl Scene {
    let color = if enabled { tint } else { DISABLED_TINT };
    bsn! {
        ImageNode { color: {color} }
        Pickable { should_block_lower: {enabled}, is_hoverable: {enabled} }
    }
}

/// A soft halo in `color` around a selected card or tile. [`Color::NONE`] clears it.
pub fn glow(color: Color) -> BoxShadow {
    if color == Color::NONE {
        return BoxShadow::default();
    }
    BoxShadow::new(color, Val::ZERO, Val::ZERO, Val::ZERO, px(10))
}
