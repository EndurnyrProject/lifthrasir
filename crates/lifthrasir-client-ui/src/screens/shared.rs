//! Helpers any menu screen can use: number formatting, header pieces and the layout
//! and state helpers for the RO-dark art buttons.

use bevy::color::{Luminance, Mix};
use bevy::prelude::*;
use bevy::text::{FontSize, FontSourceTemplate, LetterSpacing};
use lifthrasir_ui::theme::{self, ro_dark};
use lifthrasir_ui::widgets::chrome::{chrome_text, glyph_icon, ignore_picking};
use lifthrasir_ui::widgets::primitives::{BUTTON_SHADOW, ButtonArt, button, button_with};

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

/// A square art button `size` px wide showing the `ui/icons/<icon>.svg` glyph, laid out
/// by its shape like [`flush_button`]. The art's corners need `size` to be 16 or more.
pub fn small_button(icon: &'static str, size: f32) -> impl Scene {
    let glyph = (size * 0.42).round();
    let size = size + 2.0 * BUTTON_SHADOW;
    bsn! {
        @button_with(ButtonArt::NORMAL, glyph_icon(icon, glyph, ro_dark::BUTTON_LABEL))
        Node {
            width: px(size),
            height: px(size),
            flex_grow: 0.0,
            flex_shrink: 0.0,
            margin: {UiRect::all(px(-BUTTON_SHADOW))},
        }
    }
}

/// A small spaced-out caption over a value or a panel.
pub fn overline(text: String, color: Color) -> impl Scene {
    let spacing = LetterSpacing::Px(1.2);
    bsn! {
        @chrome_text(text, 10.0, color)
        spacing
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

/// `Inherited` when `shown`, else `Hidden`.
pub fn shown_if(shown: bool) -> Visibility {
    if shown {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    }
}

/// The outline of an item or skill slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ring {
    None,
    Rare,
    Selected,
}

impl Ring {
    /// Stroke colour, stroke width and halo.
    pub fn style(self) -> (Color, f32, BoxShadow) {
        match self {
            Self::None => (ro_dark::SLOT_STROKE, 1.0, glow(Color::NONE)),
            Self::Rare => (
                ro_dark::RARE_RING,
                2.0,
                glow(ro_dark::RARE_GLOW.with_alpha(0.5)),
            ),
            Self::Selected => (
                ro_dark::SELECT_RING,
                2.0,
                glow(ro_dark::FROST_GLOW.with_alpha(0.55)),
            ),
        }
    }
}

/// The square every slot is drawn in, centring its icon.
pub fn slot_frame(size: f32, ring: Ring) -> impl Scene {
    let (stroke, width, halo) = ring.style();
    bsn! {
        Node {
            width: px(size),
            height: px(size),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: px(width),
        }
        BackgroundColor(ro_dark::SLOT)
        BorderColor::all(stroke)
        halo
    }
}

/// A stack of text lines, each split into coloured runs: a `Text` holding the first
/// run plus a `TextSpan` per following one.
pub fn colored_lines(lines: Vec<Vec<(Color, String)>>) -> impl Scene {
    let rows: Vec<_> = lines.into_iter().map(colored_line).collect();
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(4) }
        @ignore_picking()
        Children [ {rows} ]
    }
}

fn colored_line(runs: Vec<(Color, String)>) -> impl Scene {
    let mut runs = runs.into_iter();
    let (first_color, first_text) = runs.next().unwrap_or((theme::TEXT_DIM, String::new()));
    let spans: Vec<_> = runs
        .map(|(color, text)| colored_span(color, text))
        .collect();
    bsn! {
        Text(first_text)
        TextFont {
            font: FontSourceTemplate::Handle(theme::FONT_BODY),
            font_size: {FontSize::Px(12.0)},
        }
        TextColor(first_color)
        @ignore_picking()
        Children [ {spans} ]
    }
}

fn colored_span(color: Color, text: String) -> impl Scene {
    bsn! {
        TextSpan(text)
        TextFont {
            font: FontSourceTemplate::Handle(theme::FONT_BODY),
            font_size: {FontSize::Px(12.0)},
        }
        TextColor(color)
    }
}

/// [`colored_lines`] recoloured with [`slate_ink`] to stay readable on the RO-dark
/// window body and its insets.
pub fn slate_lines(lines: Vec<Vec<(Color, String)>>) -> impl Scene {
    let lines = lines
        .into_iter()
        .map(|runs| {
            runs.into_iter()
                .map(|(color, text)| (slate_ink(color), text))
                .collect()
        })
        .collect();
    colored_lines(lines)
}

/// RO colour codes were authored for a white window, so dark hues are mixed towards
/// white until they reach 4.5:1 contrast on the RO-dark window body.
pub fn slate_ink(color: Color) -> Color {
    if color == theme::TEXT_DIM {
        return ro_dark::BODY;
    }
    let body = ro_dark::WINDOW_BODY.to_linear().luminance();
    let min_luminance = (body + 0.05) * 4.6 - 0.05;
    let linear = color.to_linear();
    let luminance = linear.luminance();
    if luminance >= min_luminance {
        return color;
    }
    let lift = (min_luminance - luminance) / (1.0 - luminance);
    linear.mix(&LinearRgba::WHITE, lift).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slate_ink_lifts_dark_hues_and_keeps_bright_ones() {
        let body = ro_dark::WINDOW_BODY.to_linear().luminance();
        let bright = Color::srgb_u8(0xff, 0xff, 0x00);
        assert_eq!(slate_ink(bright), bright);
        assert_eq!(slate_ink(lifthrasir_ui::theme::TEXT_DIM), ro_dark::BODY);
        for dark in [
            Color::srgb_u8(0x00, 0x00, 0x88),
            Color::srgb_u8(0xcc, 0x00, 0x00),
            Color::srgb_u8(0x77, 0x77, 0x77),
            Color::BLACK,
        ] {
            let ink = slate_ink(dark).to_linear().luminance();
            let contrast = (ink + 0.05) / (body + 0.05);
            assert!(contrast >= 4.5, "{dark:?}: contrast {contrast}");
        }
    }
}
