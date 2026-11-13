//! How the status bar is drawn: the full and the minified plate, their tooltip and
//! the level-up banner. The values are blank here; `refresh` writes them in.

use bevy::color::Mix;
use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::LineHeight;
use bevy::ui::ShadowStyle;
use bevy::ui_widgets::Button;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{chrome_text, ignore_picking};
use lifthrasir_ui::widgets::primitives::display_text;

use super::portrait::Portrait;
use super::view::{Field, Gauge, LevelUp, Tip, TipView};
use super::{
    ApRow, Banner, BarAction, Fill, LowPulse, Plate, PlateHost, PortraitRing, StatusBarRoot,
    StatusTip, Trail, hide_tip, on_bar_button, show_tip,
};
use crate::screens::shared::{overline, plate_button, tooltip};
use crate::widgets::status_icons;

/// Gap between the bar and the screen's top-left corner.
pub(super) const MARGIN: f32 = 16.0;
/// Gap between the plate and what sits beside or under it.
const GAP: f32 = 12.0;
const FULL: Vec2 = Vec2::new(560.0, 124.0);
const MINI: Vec2 = Vec2::new(344.0, 50.0);
/// The pale bar a spent resource leaves behind for a moment.
const GHOST: Color = Color::srgba(1.0, 0.925, 0.784, 0.55);
/// Gold flash of the plate on a level-up.
const LEVEL_GLOW: Color = ro_dark::RARE_RING;

/// The plate's height, which the tooltip hangs under.
pub(super) fn plate_height(minimal: bool) -> f32 {
    if minimal { MINI.y } else { FULL.y }
}

/// Where the level-up banner sits below the screen's top edge.
pub(super) fn banner_top(minimal: bool) -> f32 {
    if minimal { 110.0 } else { 196.0 }
}

/// The bar's corner of the HUD: the plate's mount, then the status effects.
pub(super) fn root() -> impl Scene {
    bsn! {
        StatusBarRoot
        Node {
            position_type: PositionType::Absolute,
            left: px(MARGIN),
            top: px(MARGIN),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexStart,
            row_gap: px(GAP),
            column_gap: px(GAP),
        }
        @ignore_picking()
        Children [
            PlateHost
            Node { flex_shrink: 0.0 }
            @ignore_picking()
            --
            @status_icons::icon_row()
            Node { max_width: px(FULL.x) }
        ]
    }
}

/// The plate's drop shadow, with a gold halo as strong as `glow` (0 to 1).
pub(super) fn plate_shadow(glow: f32) -> BoxShadow {
    let shade = |color, y, blur| ShadowStyle {
        color,
        x_offset: Val::ZERO,
        y_offset: px(y),
        spread_radius: Val::ZERO,
        blur_radius: px(blur),
    };
    BoxShadow(vec![
        shade(Color::BLACK.with_alpha(0.6), 6, 16),
        shade(LEVEL_GLOW.with_alpha(0.8 * glow), 0, 18),
    ])
}

/// The plate's bronze edge, turned gold as far as `glow` (0 to 1).
pub(super) fn plate_edge(glow: f32) -> BorderColor {
    BorderColor::all(ro_dark::BUTTON_STROKE.mix(&LEVEL_GLOW, glow))
}

/// The slate plate both sizes are laid out on.
fn plate(size: Vec2) -> impl Scene {
    let slate = BackgroundGradient::from(LinearGradient::to_bottom(vec![
        ColorStop::new(ro_dark::TOOLTIP_BODY.with_alpha(0.92), percent(0)),
        ColorStop::new(ro_dark::INSET.with_alpha(0.92), percent(100)),
    ]));
    let rim = ro_dark::WINDOW_OUTLINE.with_alpha(0.8);
    let (shadow, edge) = (plate_shadow(0.0), plate_edge(0.0));
    bsn! {
        Plate
        Node {
            width: px(size.x),
            height: px(size.y),
            border: px(1),
            border_radius: BorderRadius::all(px(8)),
            position_type: PositionType::Relative,
        }
        slate
        edge
        Outline { width: px(1), offset: px(0), color: rim }
        shadow
        Pickable
    }
}

/// The light caught along the plate's top edge.
fn sheen() -> impl Scene {
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

/// The full plate: portrait, identity, the three bars, both EXP rows and the purse.
pub(super) fn full() -> impl Scene {
    let bars: Vec<_> = Gauge::VITALS.into_iter().map(bar).collect();
    bsn! {
        @plate(FULL)
        Node { padding: {UiRect::axes(px(11), px(9))}, column_gap: px(12) }
        Children [
            @sheen()
            --
            @avatar(72.0, true)
            Node { margin: {UiRect::top(px(4))} }
            --
            Node { flex_grow: 1.0, min_width: px(0), flex_direction: FlexDirection::Column }
            @ignore_picking()
            Children [
                @heading()
                --
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4),
                    margin: {UiRect::top(px(5))},
                }
                @ignore_picking()
                Children [ {bars} ]
                --
                Node { column_gap: px(12), margin: {UiRect::top(px(7))} }
                @ignore_picking()
                Children [ @exp(Gauge::BaseExp) -- @exp(Gauge::JobExp) ]
            ]
            --
            @purse()
        ]
    }
}

/// The minified plate: portrait, name and levels, the three bars and a base EXP sliver.
pub(super) fn mini() -> impl Scene {
    let bars: Vec<_> = Gauge::VITALS.into_iter().map(mini_bar).collect();
    // The chevron art points right; this one opens the plate downwards.
    let down = UiTransform::from_rotation(Rot2::degrees(90.0));
    bsn! {
        @plate(MINI)
        Node {
            align_items: AlignItems::Center,
            column_gap: px(9),
            padding: {UiRect::new(px(7), px(7), px(6), px(8))},
        }
        Children [
            @sheen()
            --
            @avatar(34.0, false)
            --
            Node {
                width: px(92),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                overflow: {Overflow::clip()},
            }
            @ignore_picking()
            Children [
                Field::Name
                @display_text(String::new(), 13.0, ro_dark::HERO)
                TextLayout { linebreak: LineBreak::NoWrap }
                --
                @levels("Lv", "\u{b7}", 10.0)
            ]
            --
            Node {
                flex_grow: 1.0,
                min_width: px(0),
                flex_direction: FlexDirection::Column,
                row_gap: px(3),
            }
            @ignore_picking()
            Children [ {bars} ]
            --
            @plate_button("chev")
            down
            BarAction::ToggleSize
            on(on_bar_button)
            --
            @sliver()
        ]
    }
}

/// The round portrait, which opens the Character window. `pill` hangs the base level
/// under it.
fn avatar(size: f32, pill: bool) -> impl Scene {
    let backdrop = BackgroundGradient::from(RadialGradient {
        shape: RadialGradientShape::Ellipse(percent(50), percent(50)),
        stops: vec![
            ColorStop::new(Color::srgb_u8(0x34, 0x40, 0x4a), percent(0)),
            ColorStop::new(ro_dark::INSET, percent(70)),
        ],
        ..default()
    });
    let rim = ro_dark::WINDOW_OUTLINE.with_alpha(0.8);
    let pill = pill.then(|| EntityScene(level_pill()));
    bsn! {
        Button
        BarAction::OpenCharacter
        PortraitRing
        Node {
            width: px(size),
            height: px(size),
            flex_shrink: 0.0,
            border: px(2),
            border_radius: BorderRadius::MAX,
            position_type: PositionType::Relative,
        }
        backdrop
        BorderColor::all(ro_dark::PRIMARY_STROKE)
        Outline { width: px(2), offset: px(0), color: rim }
        Pickable
        on(on_bar_button)
        Children [
            Portrait
            Node { width: percent(100), height: percent(100), border_radius: BorderRadius::MAX }
            @ignore_picking()
            --
            {pill}
        ]
    }
}

/// The base level on a pill straddling the portrait's lower edge.
fn level_pill() -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            bottom: px(-10),
            justify_content: JustifyContent::Center,
        }
        @ignore_picking()
        Children [
            Node {
                height: px(18),
                min_width: px(34),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: {UiRect::horizontal(px(6))},
                border: px(1),
                border_radius: BorderRadius::all(px(9)),
            }
            BackgroundColor(ro_dark::TOOLTIP_BODY)
            BorderColor::all(ro_dark::PRIMARY_STROKE)
            @ignore_picking()
            Children [
                Field::BaseLevel
                @chrome_text(String::new(), 11.0, ro_dark::BRONZE_LIGHT)
            ]
        ]
    }
}

/// Name and job on the left, both levels on the right.
fn heading() -> impl Scene {
    bsn! {
        Node { height: px(22), align_items: AlignItems::Baseline, column_gap: px(8) }
        @ignore_picking()
        Children [
            Field::Name
            @display_text(String::new(), 16.0, ro_dark::HERO)
            TextLayout { linebreak: LineBreak::NoWrap }
            --
            Field::Job
            @chrome_text(String::new(), 12.0, ro_dark::MUTED)
            TextLayout { linebreak: LineBreak::NoWrap }
            Node {
                flex_grow: 1.0,
                flex_basis: px(0),
                min_width: px(0),
                overflow: {Overflow::clip()},
            }
            --
            @levels("Base", "\u{b7} Job", 11.0)
        ]
    }
}

/// `lead` base level `mid` job level, as in `Base 215 · Job 32`.
fn levels(lead: &'static str, mid: &'static str, size: f32) -> impl Scene {
    bsn! {
        Node { flex_shrink: 0.0, column_gap: px(3) }
        @ignore_picking()
        Children [
            @chrome_text(lead.into(), size, ro_dark::FAINT)
            --
            Field::BaseLevel
            @chrome_text(String::new(), size, ro_dark::BRONZE_LIGHT)
            --
            @chrome_text(mid.into(), size, ro_dark::FAINT)
            --
            Field::JobLevel
            @chrome_text(String::new(), size, ro_dark::STATUS_ONLINE)
        ]
    }
}

/// What makes a row explain itself under the pointer.
fn hint(tip: Tip) -> impl Scene {
    bsn! {
        tip
        Pickable
        on(show_tip)
        on(hide_tip)
    }
}

/// The AP row starts hidden: only fourth jobs have AP.
fn ap_row(gauge: Gauge) -> Option<impl Scene> {
    (gauge == Gauge::Ap).then_some(bsn! { ApRow Node { display: Display::None } })
}

/// A bar of the full plate: label, track, then amount over limit.
fn bar(gauge: Gauge) -> impl Scene {
    let height = match gauge {
        Gauge::Hp => 12.0,
        Gauge::Sp => 9.0,
        _ => 7.0,
    };
    let (value, max) = (Field::Value(gauge), Field::Max(gauge));
    let ap = ap_row(gauge);
    bsn! {
        @hint(Tip::Gauge(gauge))
        Node { align_items: AlignItems::Center, column_gap: px(7) }
        @{ap}
        Children [
            @chrome_text(gauge.label().into(), 10.0, gauge.ink())
            Node { width: px(20), flex_shrink: 0.0 }
            --
            @track(gauge, height)
            --
            Node {
                width: px(108),
                flex_shrink: 0.0,
                justify_content: JustifyContent::FlexEnd,
                column_gap: px(3),
            }
            @ignore_picking()
            Children [
                value
                @chrome_text(String::new(), 11.0, ro_dark::STRONG)
                --
                max
                @chrome_text(String::new(), 11.0, ro_dark::FAINT)
            ]
        ]
    }
}

/// A bar of the minified plate: track, then a percentage (the count, for AP).
fn mini_bar(gauge: Gauge) -> impl Scene {
    let height = match gauge {
        Gauge::Hp => 6.0,
        Gauge::Sp => 5.0,
        _ => 4.0,
    };
    let short = Field::Short(gauge);
    let ap = ap_row(gauge);
    let leading = LineHeight::Px(10.0);
    bsn! {
        @hint(Tip::Gauge(gauge))
        Node { align_items: AlignItems::Center, column_gap: px(6) }
        @{ap}
        Children [
            @track(gauge, height)
            --
            short
            @chrome_text(String::new(), 9.0, gauge.ink())
            leading
            TextLayout { justify: Justify::Right, linebreak: LineBreak::NoWrap }
            Node { width: px(30), flex_shrink: 0.0 }
        ]
    }
}

/// An EXP row: caption and percentage over a hairline track.
fn exp(gauge: Gauge) -> impl Scene {
    let value = Field::Value(gauge);
    bsn! {
        @hint(Tip::Gauge(gauge))
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            flex_direction: FlexDirection::Column,
            row_gap: px(2),
        }
        Children [
            Node { justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center }
            @ignore_picking()
            Children [
                @overline(gauge.label().into(), ro_dark::FAINT)
                --
                value
                @chrome_text(String::new(), 9.0, gauge.ink())
            ]
            --
            @track(gauge, 3.0)
            Node { flex_grow: 0.0 }
        ]
    }
}

/// The right-hand column: the two buttons, zeny and carried weight.
fn purse() -> impl Scene {
    let rule = BorderColor::all(ro_dark::DIVIDER.with_alpha(0.35));
    // The chevron art points right; this one folds the plate upwards.
    let up = UiTransform::from_rotation(Rot2::degrees(-90.0));
    let (share, amount) = (Field::Short(Gauge::Weight), Field::Value(Gauge::Weight));
    bsn! {
        Node {
            width: px(124),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            padding: {UiRect::left(px(12))},
            border: {UiRect::left(px(1))},
        }
        rule
        @ignore_picking()
        Children [
            Node { justify_content: JustifyContent::FlexEnd, column_gap: px(6) }
            @ignore_picking()
            Children [
                @plate_button("user")
                BarAction::OpenCharacter
                on(on_bar_button)
                --
                @plate_button("chev")
                up
                BarAction::ToggleSize
                on(on_bar_button)
            ]
            --
            @hint(Tip::Zeny)
            Node { flex_direction: FlexDirection::Column, margin: {UiRect::top(px(2))} }
            Children [
                @overline("ZENY".into(), ro_dark::FAINT)
                --
                Field::Zeny
                @chrome_text(String::new(), 13.0, ro_dark::BRONZE_LIGHT)
            ]
            --
            @hint(Tip::Gauge(Gauge::Weight))
            Node { flex_direction: FlexDirection::Column, row_gap: px(2) }
            Children [
                Node {
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                }
                @ignore_picking()
                Children [
                    @overline(Gauge::Weight.label().into(), ro_dark::FAINT)
                    --
                    share
                    @chrome_text(String::new(), 9.0, Gauge::Weight.ink())
                ]
                --
                @track(Gauge::Weight, 4.0)
                Node { flex_grow: 0.0 }
                --
                amount
                @chrome_text(String::new(), 10.0, ro_dark::MUTED)
            ]
        ]
    }
}

/// The dark ends of the three vital bars; the bar brightens towards its tip.
fn vital_shade(gauge: Gauge) -> Option<(Color, Color)> {
    match gauge {
        Gauge::Hp => Some((
            Color::srgb_u8(0x8e, 0x3a, 0x33),
            Color::srgb_u8(0xc4, 0x55, 0x4b),
        )),
        Gauge::Sp => Some((
            Color::srgb_u8(0x2c, 0x5a, 0x8a),
            Color::srgb_u8(0x4a, 0x8a, 0xc8),
        )),
        Gauge::Ap => Some((
            Color::srgb_u8(0x9a, 0x6f, 0x2c),
            Color::srgb_u8(0xe0, 0xb2, 0x57),
        )),
        _ => None,
    }
}

/// Something stretched from the track's left end; a system sets how far.
fn layer() -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            bottom: px(0),
            width: percent(0),
            border_radius: BorderRadius::all(px(3)),
        }
        @ignore_picking()
    }
}

/// A gauge's trough and fill, growing to share its row. The vital bars also get the
/// ghost of what was just spent, and the weight bar its 50% and 90% marks.
fn track(gauge: Gauge, height: f32) -> impl Scene {
    let fill = Fill(gauge);
    let (solid, gradient) = match vital_shade(gauge) {
        Some((from, to)) => {
            let gradient = BackgroundGradient::from(LinearGradient::to_right(vec![
                ColorStop::new(from, percent(0)),
                ColorStop::new(to, percent(100)),
            ]));
            (to, Some(bsn! { gradient }))
        }
        None => (gauge.ink(), None),
    };
    let gloss = BackgroundGradient::from(LinearGradient::to_bottom(vec![
        ColorStop::new(Color::WHITE.with_alpha(0.28), percent(0)),
        ColorStop::new(Color::WHITE.with_alpha(0.0), percent(100)),
    ]));
    let ghost = vital_shade(gauge).map(|_| {
        let trail = Trail {
            gauge,
            shown: 0.0,
            hold: 0.0,
        };
        EntityScene(bsn! { trail @layer() BackgroundColor(GHOST) })
    });
    let pulse = (gauge == Gauge::Hp).then(|| {
        EntityScene(bsn! {
            LowPulse
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100) }
            BackgroundColor(Color::NONE)
            @ignore_picking()
        })
    });
    let marks: Vec<_> = [50.0, 90.0]
        .into_iter()
        .filter(|_| gauge == Gauge::Weight)
        .map(mark)
        .collect();
    let trough = Color::BLACK.with_alpha(0.5);
    bsn! {
        Node {
            flex_grow: 1.0,
            min_width: px(0),
            height: px(height),
            border_radius: BorderRadius::all(px(3)),
            overflow: {Overflow::clip()},
            position_type: PositionType::Relative,
        }
        BackgroundColor(trough)
        @ignore_picking()
        Children [
            {ghost}
            --
            fill
            @layer()
            BackgroundColor(solid)
            @{gradient}
            Children [
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(45),
                }
                gloss
                @ignore_picking()
                --
                {pulse}
            ]
            --
            {marks}
        ]
    }
}

/// A hairline across the weight bar at one of RO's weight tiers.
fn mark(at: f32) -> impl Scene {
    let ink = Color::BLACK.with_alpha(0.7);
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: {Val::Percent(at)},
            top: px(0),
            bottom: px(0),
            width: px(1),
        }
        BackgroundColor(ink)
        @ignore_picking()
    }
}

/// Base EXP along the minified plate's bottom edge. It stops short of the plate's
/// round corners, which nothing clips it to.
fn sliver() -> impl Scene {
    let fill = Fill(Gauge::BaseExp);
    let trough = Color::BLACK.with_alpha(0.55);
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(6),
            right: px(6),
            bottom: px(0),
            height: px(3),
            overflow: {Overflow::clip()},
        }
        BackgroundColor(trough)
        @ignore_picking()
        Children [
            fill
            @layer()
            BackgroundColor(ro_dark::BASE_EXP)
        ]
    }
}

/// The tooltip of a hovered part, hung under the plate `left` px along the bar.
pub(super) fn tip(view: TipView, left: f32, top: f32) -> impl Scene {
    bsn! {
        StatusTip
        @tooltip(view.title, view.tone, view.lines)
        Node { position_type: PositionType::Absolute, left: px(left), top: px(top) }
    }
}

/// The level-up banner, centred across the screen `top` px down.
pub(super) fn banner(level: LevelUp, top: f32) -> impl Scene {
    let (kicker, headline) = level.lines();
    let spacing = bevy::text::LetterSpacing::Px(1.6);
    bsn! {
        Banner
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(top),
            width: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
        }
        @ignore_picking()
        Children [
            @chrome_text(kicker.into(), 10.0, LEVEL_GLOW)
            spacing
            TextShadow { offset: Vec2::splat(1.0), color: Color::BLACK }
            --
            @display_text(headline, 26.0, ro_dark::HERO)
            TextShadow { offset: Vec2::splat(2.0), color: Color::BLACK }
        ]
    }
}
