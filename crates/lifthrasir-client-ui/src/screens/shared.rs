//! Helpers any menu screen can use: number formatting, header pieces and the layout
//! and state helpers for the RO-dark art buttons.

use std::time::Duration;

use bevy::color::{Luminance, Mix};
use bevy::input_focus::InputFocus;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{
    EditableText, EditableTextFilter, FontSize, FontSourceTemplate, LetterSpacing, TextEdit,
};
use bevy::ui::ShadowStyle;
use bevy::ui_widgets::{Button, TextInput};
use lifthrasir_client::domain::settings::{KeyBind, Modifier};
use lifthrasir_ui::theme::{self, ro_dark};
use lifthrasir_ui::widgets::chrome::{chrome_text, glyph_icon, ignore_picking};
use lifthrasir_ui::widgets::placeholder::Placeholder;
use lifthrasir_ui::widgets::primitives::{
    BUTTON_SHADOW, ButtonArt, HoverColors, INPUT_SHADOW, PRIMITIVES_DIR, button, button_with,
    input_frame,
};

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

/// A [`flush_button`] whose shape is `width` px wide, for footers and prompts.
pub fn wide_button(art: ButtonArt, label: &'static str, width: f32) -> impl Scene {
    bsn! {
        @flush_button(art, label)
        Node { width: px(width + 2.0 * BUTTON_SHADOW), flex_grow: 0.0, flex_shrink: 0.0 }
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

/// A 24px-high art button `width` px wide, for list rows and inline prompts. It is
/// laid out by its shape like [`flush_button`].
pub fn row_button(art: ButtonArt, label: impl Into<String>, width: f32) -> impl Scene {
    bsn! {
        @button_with(art, chrome_text(label.into(), 11.0, ro_dark::BUTTON_LABEL))
        Node {
            width: px(width + 2.0 * BUTTON_SHADOW),
            height: px(24.0 + 2.0 * BUTTON_SHADOW),
            flex_grow: 0.0,
            flex_shrink: 0.0,
            margin: {UiRect::all(px(-BUTTON_SHADOW))},
        }
    }
}

/// An art button as wide as `content` plus 14px on each side, for labels only known at
/// runtime. It is laid out by its shape like [`flush_button`].
pub fn fit_button(art: ButtonArt, content: impl Scene) -> impl Scene {
    bsn! {
        @button_with(art, content)
        Node {
            flex_grow: 0.0,
            flex_shrink: 0.0,
            padding: {UiRect::horizontal(px(BUTTON_SHADOW + 14.0))},
            margin: {UiRect::all(px(-BUTTON_SHADOW))},
        }
    }
}

/// How a [`menu_row`] is painted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowTone {
    Plain,
    Primary,
    /// Plain at rest and oxblood under the pointer, for what ends the session.
    Danger,
    /// No fill at rest.
    Ghost,
}

impl RowTone {
    /// The `(fill, stroke)` pairs at rest and under the pointer.
    pub fn colors(self) -> HoverColors {
        let plain = (ro_dark::TAB_INACTIVE, ro_dark::SLOT_STROKE);
        let (rest, hover) = match self {
            Self::Plain => (plain, (ro_dark::TAB_HOVER, ro_dark::PRIMARY_STROKE)),
            Self::Primary => (
                (ro_dark::PRIMARY, ro_dark::PRIMARY_STROKE),
                (ro_dark::PRIMARY_HOVER, ro_dark::PRIMARY_STROKE_HOVER),
            ),
            Self::Danger => (plain, (ro_dark::DANGER_PRESSED, ro_dark::BRONZE)),
            Self::Ghost => (
                (Color::NONE, ro_dark::SLOT_STROKE),
                (Color::WHITE.with_alpha(0.04), ro_dark::BUTTON_STROKE),
            ),
        };
        HoverColors { rest, hover }
    }
}

/// A full-width row of a menu: the `ui/icons/<icon>.svg` glyph, a label and the key
/// that also triggers it. The caller attaches the `Activate` observer.
pub fn menu_row(
    tone: RowTone,
    height: f32,
    icon: &'static str,
    label: &'static str,
    key: Option<&'static str>,
) -> impl Scene {
    let colors = tone.colors();
    let (fill, stroke) = colors.rest;
    let ink = if tone == RowTone::Ghost {
        ro_dark::LABEL
    } else {
        ro_dark::BUTTON_LABEL
    };
    let key = key.map(|key| EntityScene(key_cap(key)));
    bsn! {
        Button
        Hovered
        colors
        Node {
            height: px(height),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            column_gap: px(12),
            padding: {UiRect::horizontal(px(14))},
            border: px(1),
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor(fill)
        BorderColor::all(stroke)
        Pickable
        Children [
            @glyph_icon(icon, 16.0, ink)
            --
            @chrome_text(label.into(), 14.0, ink)
            Node { flex_grow: 1.0 }
            --
            {key}
        ]
    }
}

/// A screen-wide layer at stacking order `z` that centres a modal and keeps clicks off
/// everything under it. The caller adds the shade.
pub fn modal_layer(z: i32) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
        }
        GlobalZIndex({z})
        Pickable
    }
}

/// A shade that is `center` up to `clear` percent of the way out, then darkens to
/// `edge` in the screen's corners.
pub fn vignette(center: Color, edge: Color, clear: f32) -> BackgroundGradient {
    BackgroundGradient::from(RadialGradient::new(
        UiPosition::CENTER,
        RadialGradientShape::FarthestCorner,
        vec![
            ColorStop::new(center, percent(clear)),
            ColorStop::new(edge, percent(100)),
        ],
    ))
}

/// The key that also triggers a button, drawn beside its label.
pub fn key_cap(key: &'static str) -> impl Scene {
    bsn! {
        Node {
            min_width: px(16),
            height: px(16),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: {UiRect::horizontal(px(4))},
            border_radius: BorderRadius::all(px(3)),
        }
        BackgroundColor({Color::BLACK.with_alpha(0.25)})
        @ignore_picking()
        Children [ @chrome_text(key.into(), 10.0, ro_dark::BUTTON_LABEL) ]
    }
}

/// A 22px framed icon button for HUD plates, showing the `ui/icons/<icon>.svg` glyph.
/// The caller attaches the `Activate` observer.
pub fn plate_button(icon: &'static str) -> impl Scene {
    let colors = HoverColors {
        rest: (ro_dark::TAB_INACTIVE, ro_dark::MUTED_BRONZE),
        hover: (ro_dark::BUTTON, ro_dark::BRONZE_HIGHLIGHT),
    };
    bsn! {
        Button
        Hovered
        colors
        Node {
            width: px(22),
            height: px(22),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: px(1),
            border_radius: BorderRadius::all(px(4)),
        }
        BackgroundColor(ro_dark::TAB_INACTIVE)
        BorderColor::all(ro_dark::MUTED_BRONZE)
        Pickable
        Children [ @glyph_icon(icon, 12.0, ro_dark::LABEL) ]
    }
}

/// Stacking order of hover tooltips, above every window.
const TOOLTIP_Z: i32 = 1500;

/// The box a hover tooltip is written in. The owner places it and fills it.
pub fn tooltip_frame() -> impl Scene {
    let shadow = BoxShadow::new(
        Color::BLACK.with_alpha(0.7),
        Val::ZERO,
        px(3),
        Val::ZERO,
        px(8),
    );
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            max_width: px(260),
            padding: {UiRect::axes(px(10), px(7))},
            border: px(1),
            border_radius: BorderRadius::all(px(5)),
        }
        BackgroundColor(ro_dark::TOOLTIP_BODY)
        BorderColor::all(ro_dark::BUTTON_STROKE)
        shadow
        GlobalZIndex(TOOLTIP_Z)
        @ignore_picking()
    }
}

/// A hover tooltip: a `title` in its `tone` over plain `lines`. The owner places it.
pub fn tooltip(title: String, tone: Color, lines: Vec<String>) -> impl Scene {
    let lines: Vec<_> = lines
        .into_iter()
        .map(|line| chrome_text(line, 11.0, ro_dark::LABEL))
        .collect();
    bsn! {
        @tooltip_frame()
        Children [
            @chrome_text(title, 12.0, tone)
            --
            {lines}
        ]
    }
}

/// A small filled tag beside a name: a count or a warning.
pub fn chip(text: String, fill: Color, ink: Color) -> impl Scene {
    bsn! {
        Node {
            height: px(18),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            padding: {UiRect::horizontal(px(6))},
            border_radius: BorderRadius::all(px(3)),
        }
        BackgroundColor(fill)
        @ignore_picking()
        Children [ @chrome_text(text, 10.0, ink) ]
    }
}

/// A search box in the input frame, growing to share its row. `F` marks the text so
/// the owner can read it, and `hint` shows while it is empty. The negative margin
/// cancels the frame's baked shadow so the row lays out by the shape.
pub fn search_field<F>(hint: &'static str) -> impl Scene
where
    F: Component + Default + Clone + Unpin,
{
    bsn! {
        @input_frame()
        Node {
            flex_grow: 1.0,
            min_width: px(0),
            margin: {UiRect::all(px(-INPUT_SHADOW))},
            position_type: PositionType::Relative,
        }
        Children [
            #field
            F::default()
            EditableText
            TextInput
            TextFont {
                font: FontSourceTemplate::Handle(theme::FONT_BODY),
                font_size: {FontSize::Px(13.0)},
            }
            TextColor(ro_dark::BODY)
            Node { flex_grow: 1.0, min_width: px(0), height: px(18) }
            --
            Placeholder(#field)
            @chrome_text(hint.into(), 13.0, ro_dark::MUTED)
            Node {
                position_type: PositionType::Absolute,
                left: px(INPUT_SHADOW + 10.0),
                top: px(INPUT_SHADOW + 6.0),
            }
        ]
    }
}

/// Lights the frame of a [`search_field`] while its `F` text has keyboard focus.
pub fn light_search_focus<F: Component>(
    focus: Res<InputFocus>,
    fields: Query<(Entity, &ChildOf), With<F>>,
    mut frames: Query<&mut Outline>,
) {
    for (field, frame) in &fields {
        let Ok(mut outline) = frames.get_mut(frame.parent()) else {
            continue;
        };
        let color = if focus.get() == Some(field) {
            ro_dark::FROST_GLOW
        } else {
            Color::NONE
        };
        if outline.color != color {
            outline.color = color;
        }
    }
}

/// A `width` px box showing the current choice of a short list over a chevron. The
/// caller adds the click observer that steps to the next choice, and `label` is its
/// own text, marked if it is rewritten in place.
pub fn cycle_box(label: impl Scene, width: f32) -> impl Scene {
    let colors = HoverColors {
        rest: (ro_dark::INSET, ro_dark::SLOT_STROKE),
        hover: (ro_dark::INSET, ro_dark::BUTTON_STROKE),
    };
    bsn! {
        Hovered
        colors
        Node {
            width: px(width),
            height: px(28),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: {UiRect::horizontal(px(10))},
            border: px(1),
            border_radius: BorderRadius::all(px(5)),
        }
        BackgroundColor(ro_dark::INSET)
        BorderColor::all(ro_dark::SLOT_STROKE)
        Pickable
        Children [
            @label
            --
            @glyph_icon("chev", 10.0, ro_dark::PRIMARY_STROKE)
        ]
    }
}

/// A stack never holds more than 30,000, so five digits cover every quantity.
pub const QTY_DIGITS: usize = 5;

/// A bordered field for a number of at most `digits` digits, starting at `value`. The
/// caller adds its marker, width and text alignment.
pub fn number_field(value: String, digits: usize) -> impl Scene {
    bsn! {
        @text_field(value, digits)
        EditableTextFilter::new(|c| c.is_ascii_digit())
    }
}

/// A bordered one-line field of at most `max_chars` characters, starting at `value`.
/// The caller adds its marker and width; [`light_field_focus`] rings it on focus.
pub fn text_field(value: String, max_chars: usize) -> impl Scene {
    let field = EditableText {
        max_characters: Some(max_chars),
        ..EditableText::new(value)
    };
    bsn! {
        field
        TextInput
        TextFont {
            font: FontSourceTemplate::Handle(theme::FONT_BODY),
            font_size: {FontSize::Px(13.0)},
        }
        TextColor(ro_dark::BODY)
        Node {
            height: px(24),
            flex_shrink: 0.0,
            padding: {UiRect::axes(px(4), px(2))},
            border: px(1),
            border_radius: BorderRadius::all(px(5)),
        }
        BackgroundColor(ro_dark::INSET)
        BorderColor::all(ro_dark::SLOT_STROKE)
    }
}

/// The number typed in a field, or `None` while it is empty or too large.
pub fn typed_number(field: &EditableText) -> Option<u32> {
    field.value().to_string().parse().ok()
}

/// Replaces a field's text and leaves the caret at its end.
pub fn set_field_text(field: &mut EditableText, text: &str) {
    field.clear();
    field.editor.set_text(text);
    field.queue_edit(TextEdit::TextEnd(false));
}

/// Rings the `F` field that has keyboard focus.
pub fn light_field_focus<F: Component>(
    focus: Res<InputFocus>,
    mut fields: Query<(Entity, &mut BorderColor), With<F>>,
) {
    if !focus.is_changed() {
        return;
    }
    for (field, mut border) in &mut fields {
        let color = if focus.get() == Some(field) {
            ro_dark::FROST_GLOW
        } else {
            ro_dark::SLOT_STROKE
        };
        *border = BorderColor::all(color);
    }
}

/// The title of a [`shop_sign`], kept on one line.
pub fn sign_title(title: String) -> impl Scene {
    bsn! {
        @chrome_text(title, 12.0, ro_dark::STRONG)
        TextLayout { linebreak: LineBreak::NoWrap }
        TextShadow { offset: Vec2::splat(1.0), color: Color::BLACK }
    }
}

/// A vending shop's sign: a bag glyph and a [`sign_title`] clipped at `max_width`.
/// `glow` halos the plate and [`Color::NONE`] leaves only its drop shadow. Picking is
/// left to the caller.
pub fn shop_sign(title: impl Scene, max_width: f32, glow: Color) -> impl Scene {
    let shade = |color, y, blur| ShadowStyle {
        color,
        x_offset: Val::ZERO,
        y_offset: px(y),
        spread_radius: Val::ZERO,
        blur_radius: px(blur),
    };
    let shadow = BoxShadow(vec![
        shade(Color::BLACK.with_alpha(0.7), 3, 8),
        shade(glow, 0, 10),
    ]);
    bsn! {
        Node {
            height: px(28),
            max_width: px(max_width),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            column_gap: px(7),
            padding: {UiRect::new(px(7), px(11), px(0), px(0))},
            border: px(1),
            border_radius: BorderRadius::all(px(4)),
        }
        BackgroundColor(ro_dark::TITLE_BAR)
        BorderColor::all(ro_dark::BRONZE)
        shadow
        Children [
            @glyph_icon("bag", 14.0, ro_dark::BASE_EXP)
            Node { flex_shrink: 0.0 }
            --
            Node { min_width: px(0), overflow: {Overflow::clip()} }
            @ignore_picking()
            Children [ @title ]
        ]
    }
}

/// RO's weight tiers: natural regen stops at 50% of the limit, attacks and skills at 90%.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Load {
    Light,
    Heavy,
    Overweight,
}

impl Load {
    pub fn of(weight: u32, max_weight: u32) -> Self {
        let (weight, max_weight) = (u64::from(weight), u64::from(max_weight));
        if max_weight == 0 {
            return Self::Light;
        }
        if weight * 10 >= max_weight * 9 {
            return Self::Overweight;
        }
        if weight * 2 >= max_weight {
            return Self::Heavy;
        }
        Self::Light
    }

    /// Status text, its colour and the bar colour.
    fn style(self) -> (&'static str, Color, Color) {
        match self {
            Self::Light => ("Light", ro_dark::STATUS_ONLINE, ro_dark::JOB_EXP),
            Self::Heavy => (
                "Heavy \u{b7} No regen",
                ro_dark::STATUS_BUSY,
                ro_dark::BASE_EXP,
            ),
            Self::Overweight => (
                "Overweight \u{b7} Can\u{2019}t attack",
                ro_dark::STATUS_FULL,
                ro_dark::HP,
            ),
        }
    }
}

/// Carried weight over the limit, with marks at the 50% and 90% tiers. It grows to
/// share its row.
pub fn weight_meter(weight: u32, max_weight: u32) -> impl Scene {
    let (status, tone, bar) = Load::of(weight, max_weight).style();
    meter(
        "Weight",
        (weight, max_weight),
        (status.into(), tone),
        bar,
        true,
    )
}

/// An `amount` over its limit: the label and both numbers, a `status` in its tone on
/// the right, then the bar. `tiers` marks RO's 50% and 90% weight tiers. It grows to
/// share its row.
pub fn meter(
    label: &'static str,
    (amount, limit): (u32, u32),
    (status, tone): (String, Color),
    color: Color,
    tiers: bool,
) -> impl Scene {
    let fill = (amount as f32 / limit.max(1) as f32).min(1.0) * 100.0;
    let amount = format!("{} / {}", thousands(amount.into()), thousands(limit.into()));
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            min_width: px(0),
            row_gap: px(6),
        }
        @ignore_picking()
        Children [
            Node { align_items: AlignItems::Baseline, column_gap: px(8) }
            @ignore_picking()
            Children [
                @chrome_text(label.into(), 12.0, ro_dark::LABEL)
                --
                @chrome_text(amount, 12.0, ro_dark::STRONG)
                Node { flex_grow: 1.0 }
                --
                @chrome_text(status, 11.0, tone)
            ]
            --
            @bar(fill, color, tiers)
        ]
    }
}

/// A trough filled to `fill` percent in `color`. `tiers` marks RO's 50% and 90%
/// weight tiers. The caller sizes it along its row.
pub fn bar(fill: f32, color: Color, tiers: bool) -> impl Scene {
    let marks: Vec<_> = [50.0, 90.0]
        .into_iter()
        .filter(|_| tiers)
        .map(tier_mark)
        .collect();
    bsn! {
        Node {
            height: px(10),
            padding: px(1),
            border: px(1),
            border_radius: BorderRadius::all(px(5)),
            position_type: PositionType::Relative,
        }
        BackgroundColor(ro_dark::BAR_TROUGH)
        BorderColor::all(ro_dark::BAR_FRAME)
        @ignore_picking()
        Children [
            Node {
                width: {Val::Percent(fill)},
                height: percent(100),
                border_radius: BorderRadius::all(px(3)),
            }
            BackgroundColor(color)
            @ignore_picking()
            --
            {marks}
        ]
    }
}

fn tier_mark(at: f32) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: {Val::Percent(at)},
            top: px(-3),
            width: px(1),
            height: px(14),
        }
        BackgroundColor({ro_dark::TITLE.with_alpha(0.55)})
        @ignore_picking()
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
    colored_text(runs, theme::FONT_BODY, 12.0)
}

/// One text split into coloured runs, in `font` at `size` px: a `Text` holding the
/// first run plus a `TextSpan` per following one.
pub fn colored_text(runs: Vec<(Color, String)>, font: &'static str, size: f32) -> impl Scene {
    let mut runs = runs.into_iter();
    let (first_color, first_text) = runs.next().unwrap_or((theme::TEXT_DIM, String::new()));
    let spans: Vec<_> = runs
        .map(|(color, text)| colored_span(color, text, font, size))
        .collect();
    bsn! {
        Text(first_text)
        TextFont {
            font: FontSourceTemplate::Handle(font),
            font_size: {FontSize::Px(size)},
        }
        TextColor(first_color)
        @ignore_picking()
        Children [ {spans} ]
    }
}

fn colored_span(color: Color, text: String, font: &'static str, size: f32) -> impl Scene {
    bsn! {
        TextSpan(text)
        TextFont {
            font: FontSourceTemplate::Handle(font),
            font_size: {FontSize::Px(size)},
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

/// An inviter's job, then what else is known about them, on one line.
pub fn about_line(job: Option<String>, about: String) -> Option<EntityScene<impl Scene>> {
    let mut runs = Vec::new();
    if let Some(job) = job {
        runs.push((ro_dark::BRONZE_LIGHT, job));
    }
    if !about.is_empty() {
        let lead = if runs.is_empty() { "" } else { " \u{b7} " };
        runs.push((ro_dark::MUTED, format!("{lead}{about}")));
    }
    (!runs.is_empty()).then(|| EntityScene(colored_lines(vec![runs])))
}

/// Which part of an invitation's countdown a node shows.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum InviteClock {
    #[default]
    Bar,
    Label,
}

/// The nodes of an [`invite_clock`], as [`tick_invite_clock`] writes them.
pub type InviteClockNode<'a> = (
    &'a InviteClock,
    &'a mut Node,
    Option<&'a mut BackgroundColor>,
    Option<&'a mut Text>,
    Option<&'a mut TextColor>,
);

/// The countdown turns red for its last seconds.
const INVITE_LOW_SECS: f32 = 5.0;

/// An invitation's countdown: a draining bar and the seconds left. `F` marks both so
/// the owner's system drives only its own clock through [`tick_invite_clock`].
pub fn invite_clock<F>() -> impl Scene
where
    F: Component + Default + Clone + Unpin,
{
    bsn! {
        Node { align_items: AlignItems::Center, column_gap: px(10) }
        @ignore_picking()
        Children [
            Node {
                flex_grow: 1.0,
                height: px(10),
                padding: px(1),
                border: px(1),
                border_radius: BorderRadius::all(px(5)),
            }
            BackgroundColor(ro_dark::BAR_TROUGH)
            BorderColor::all(ro_dark::BAR_FRAME)
            @ignore_picking()
            Children [
                F::default()
                InviteClock::Bar
                Node {
                    width: percent(100),
                    height: percent(100),
                    border_radius: BorderRadius::all(px(3)),
                }
                BackgroundColor(ro_dark::SP)
                @ignore_picking()
            ]
            --
            Node { width: px(96), flex_shrink: 0.0, justify_content: JustifyContent::FlexEnd }
            @ignore_picking()
            Children [
                F::default()
                InviteClock::Label
                @chrome_text(String::new(), 11.0, ro_dark::MUTED)
            ]
        ]
    }
}

/// Drains the bar of the `F` [`invite_clock`] to what `timer` has left and counts the
/// seconds down beside it.
pub fn tick_invite_clock<F: Component>(
    timer: &Timer,
    clocks: &mut Query<InviteClockNode, With<F>>,
) {
    let left = timer.remaining_secs();
    let low = left <= INVITE_LOW_SECS;
    for (clock, mut node, fill, text, color) in clocks {
        match clock {
            InviteClock::Bar => {
                node.width = Val::Percent(timer.fraction_remaining() * 100.0);
                let tone = if low { ro_dark::HP } else { ro_dark::SP };
                if let Some(mut fill) = fill
                    && fill.0 != tone
                {
                    fill.0 = tone;
                }
            }
            InviteClock::Label => {
                let label = format!("Expires in {} s", left.ceil() as u32);
                if let Some(mut text) = text
                    && text.0 != label
                {
                    text.0 = label;
                }
                let tone = if low {
                    ro_dark::STATUS_FULL
                } else {
                    ro_dark::MUTED
                };
                if let Some(mut color) = color
                    && color.0 != tone
                {
                    color.0 = tone;
                }
            }
        }
    }
}

/// A checkbox that cannot be ticked yet, with its `label` greyed out.
pub fn disabled_checkbox(label: &'static str) -> impl Scene {
    bsn! {
        Node { align_items: AlignItems::Center, column_gap: px(4) }
        @ignore_picking()
        Children [
            ImageNode {
                image: {format!("{PRIMITIVES_DIR}control/checkbox_off.png")},
                color: DISABLED_TINT,
            }
            Node { width: px(34), height: px(34) }
            @ignore_picking()
            --
            @chrome_text(label.into(), 12.0, ro_dark::DISABLED_LABEL)
        ]
    }
}

/// The countdown of a timed effect: `9s`, `1:52`, `2h`.
pub fn clock(remaining: Duration) -> String {
    let secs = remaining.as_secs();
    match secs {
        3600.. => format!("{}h", secs / 3600),
        60.. => format!("{}:{:02}", secs / 60, secs % 60),
        _ => format!("{secs}s"),
    }
}

/// A length of time in seconds, from milliseconds, without trailing zeros: `2s`,
/// `1.5s`, `0.25s`.
pub fn seconds(millis: u32) -> String {
    let secs = format!("{:.2}", millis as f32 / 1000.0);
    format!("{}s", secs.trim_end_matches('0').trim_end_matches('.'))
}

/// Human-readable name for a stored key. Strips the `Key`/`Digit` prefixes the
/// `KeyCode` reflect names carry ("KeyA" → "A", "Digit1" → "1"); other names
/// pass through ("Insert", "Space", "F5").
fn key_display(name: &str) -> &str {
    name.strip_prefix("Key")
        .or_else(|| name.strip_prefix("Digit"))
        .unwrap_or(name)
}

/// The keycap label for a bind: the bound key (with modifier prefix, e.g.
/// "Alt + A"), or "—" when nothing is bound.
pub fn keycap_label(bind: &KeyBind) -> String {
    if !bind.is_bound() {
        return "—".to_string();
    }
    let key = key_display(&bind.key);
    match modifier_label(bind.modifier) {
        Some(modifier) => format!("{modifier} + {key}"),
        None => key.to_string(),
    }
}

/// Short label for a modifier in a chord display; `None` for an unmodified key.
fn modifier_label(modifier: Modifier) -> Option<&'static str> {
    let label = match modifier {
        Modifier::None => return None,
        Modifier::Alt => "Alt",
        Modifier::Control => "Ctrl",
        Modifier::Shift => "Shift",
        Modifier::Super => "Super",
    };
    Some(label)
}

/// A corner label over a slot's icon. The caller pins it with
/// `left`/`right`/`top`/`bottom`.
pub fn badge(text: String, size: f32, color: Color) -> impl Scene {
    bsn! {
        @chrome_text(text, size, color)
        TextShadow { offset: Vec2::splat(1.0), color: Color::BLACK }
        Node { position_type: PositionType::Absolute }
    }
}

/// The slate plate the HUD's bars are laid out on. The caller sizes and fills it.
pub fn hud_plate() -> impl Scene {
    let slate = BackgroundGradient::from(LinearGradient::to_bottom(vec![
        ColorStop::new(ro_dark::TOOLTIP_BODY.with_alpha(0.92), percent(0)),
        ColorStop::new(ro_dark::INSET.with_alpha(0.92), percent(100)),
    ]));
    let rim = ro_dark::WINDOW_OUTLINE.with_alpha(0.8);
    let shadow = BoxShadow::new(
        Color::BLACK.with_alpha(0.6),
        Val::ZERO,
        px(6),
        Val::ZERO,
        px(16),
    );
    bsn! {
        Node {
            border: px(1),
            border_radius: BorderRadius::all(px(8)),
            position_type: PositionType::Relative,
        }
        slate
        BorderColor::all(ro_dark::BUTTON_STROKE)
        Outline { width: px(1), offset: px(0), color: rim }
        shadow
        Pickable
    }
}

/// The light caught along the top edge of a [`hud_plate`].
pub fn plate_sheen() -> impl Scene {
    let light = BackgroundGradient::from(LinearGradient::to_right(vec![
        ColorStop::new(ro_dark::BRONZE_LIGHT.with_alpha(0.0), percent(0)),
        ColorStop::new(ro_dark::BRONZE_LIGHT.with_alpha(0.7), percent(50)),
        ColorStop::new(ro_dark::BRONZE_LIGHT.with_alpha(0.0), percent(100)),
    ]));
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(10),
            right: px(10),
            top: px(-1),
            height: px(1),
        }
        light
        @ignore_picking()
    }
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
    fn load_follows_the_fifty_and_ninety_percent_tiers() {
        assert_eq!(Load::of(999, 2000), Load::Light);
        assert_eq!(Load::of(1000, 2000), Load::Heavy);
        assert_eq!(Load::of(1799, 2000), Load::Heavy);
        assert_eq!(Load::of(1800, 2000), Load::Overweight);
        assert_eq!(Load::of(0, 0), Load::Light);
    }

    #[test]
    fn seconds_drop_trailing_zeros() {
        assert_eq!(seconds(2000), "2s");
        assert_eq!(seconds(1500), "1.5s");
        assert_eq!(seconds(250), "0.25s");
        assert_eq!(seconds(10_000), "10s");
    }

    #[test]
    fn clock_shortens_with_the_time_left() {
        assert_eq!(clock(Duration::from_secs(9)), "9s");
        assert_eq!(clock(Duration::from_secs(112)), "1:52");
        assert_eq!(clock(Duration::from_secs(2 * 3600 + 5)), "2h");
    }

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
