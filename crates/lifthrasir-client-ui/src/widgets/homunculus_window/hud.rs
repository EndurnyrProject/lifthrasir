//! The Homunculus plate on the HUD, after the `Homunculus (HUD)` artboard: the
//! companion at a glance and its four orders, shown while it is out.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::ui_widgets::Button;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{chrome_text, ignore_picking};
use lifthrasir_ui::widgets::primitives::{HoverColors, display_text, ghost_button};

use super::scene::glyph_word;
use super::view::{HudView, INTIMACY, Order};
use super::{Act, Host, Portrait, Vital, on_button};
use crate::screens::shared::{chip, hud_plate, outlined_text, plate_button, plate_sheen};
use crate::widgets::hud_layout::{HudWidget, movable};

/// NOTE: beside the party frames, under the status bar at its full size. Anchor the
/// plate to them once the HUD has a layout that can stack its widgets.
const LEFT: f32 = 282.0;
const TOP: f32 = 236.0;
const WIDTH: f32 = 290.0;
const AVATAR: f32 = 48.0;

/// Where the plate sits on the HUD. Empty unless the companion is out.
pub(super) fn host() -> impl Scene {
    bsn! {
        Host::Hud
        @movable(HudWidget::Homunculus)
        Node { position_type: PositionType::Absolute, left: px(LEFT), top: px(TOP) }
        @ignore_picking()
    }
}

/// The bronze rim, amber while the companion is hungry.
fn rim(hungry: bool) -> Color {
    if hungry {
        ro_dark::STATUS_BUSY
    } else {
        ro_dark::BUTTON_STROKE
    }
}

pub(super) fn plate(view: HudView) -> Box<dyn Scene> {
    if view.minimal {
        Box::new(portrait_plate(view.level, rim(view.hungry)))
    } else {
        Box::new(full_plate(view))
    }
}

/// The minified plate: the portrait alone, which brings the plate back on a click.
/// The slate around it is what the plate is dragged by.
fn portrait_plate(level: u32, rim: Color) -> impl Scene {
    bsn! {
        @hud_plate()
        Node { padding: {UiRect::new(px(6), px(6), px(6), px(12))} }
        BorderColor::all(rim)
        Children [ @avatar(level, rim, Act::TogglePlate) ]
    }
}

fn full_plate(view: HudView) -> impl Scene {
    let rim = rim(view.hungry);
    let lit = |order| view.order == Some(order);
    let [follow, attack, stay, feed] = view.keys;
    let auto = view.auto.map(|rule| {
        EntityScene(chip(
            rule,
            ro_dark::FROST_GLOW.with_alpha(0.1),
            ro_dark::SELECT_RING,
        ))
    });
    bsn! {
        @hud_plate()
        Node {
            width: px(WIDTH),
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
            padding: {UiRect::new(px(10), px(12), px(10), px(10))},
        }
        BorderColor::all(rim)
        Children [
            @plate_sheen()
            --
            Node { column_gap: px(10) }
            @ignore_picking()
            Children [
                @avatar(view.level, rim, Act::Open)
                --
                Node {
                    flex_grow: 1.0,
                    flex_basis: px(0),
                    min_width: px(0),
                    flex_direction: FlexDirection::Column,
                }
                @ignore_picking()
                Children [
                    Node { align_items: AlignItems::Baseline, column_gap: px(6) }
                    @ignore_picking()
                    Children [
                        @display_text(view.name, 14.0, ro_dark::HERO)
                        --
                        @chrome_text(view.species, 10.0, ro_dark::MUTED)
                        Node { flex_grow: 1.0 }
                        --
                        @glyph_word("heart", view.grade.into(), INTIMACY)
                    ]
                    --
                    @chrome_text(view.activity, 11.0, view.tone)
                    TextLayout { linebreak: LineBreak::NoWrap }
                    --
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(3),
                        margin: {UiRect::top(px(5))},
                    }
                    @ignore_picking()
                    Children [
                        @vital(Vital::HpFill, Vital::HpText, ro_dark::HP, ro_dark::FALLEN, 6.0)
                        --
                        @vital(Vital::SpFill, Vital::SpText, ro_dark::SP, ro_dark::FROST, 5.0)
                        --
                        @track(
                            bsn! {
                                Node { width: {Val::Percent(view.hunger as f32)} }
                                BackgroundColor({view.hunger_tone})
                            },
                            chrome_text(view.hunger_word.into(), 9.0, view.hunger_tone),
                            4.0,
                        )
                    ]
                ]
            ]
            --
            Node { align_items: AlignItems::Center, column_gap: px(5) }
            @ignore_picking()
            Children [
                @order("arrowin", follow, Act::Follow, lit(Order::Follow), view.out)
                --
                @order("sword", attack, Act::Attack, lit(Order::Attack), view.out)
                --
                @order("pin", stay, Act::Standby, lit(Order::Stay), view.out)
                --
                @order("apple", feed, Act::Feed, false, view.can_feed)
                --
                Node { flex_grow: 1.0 }
                @ignore_picking()
                --
                {auto}
                --
                @ghost_button("contract")
                Act::TogglePlate
                on(on_button)
            ]
        ]
    }
}

/// The companion's face over its level. A click runs `act`: the full plate opens
/// the window, the minified one brings the plate back.
fn avatar(level: u32, rim: Color, act: Act) -> impl Scene {
    let paper = BackgroundGradient::from(RadialGradient::new(
        UiPosition::CENTER,
        RadialGradientShape::ClosestSide,
        vec![
            ColorStop::new(ro_dark::HERO, percent(0)),
            ColorStop::new(ro_dark::PARCHMENT_SHADE, percent(100)),
        ],
    ));
    bsn! {
        Button
        Hovered
        Node {
            width: px(AVATAR),
            height: px(AVATAR),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: px(2),
            border_radius: BorderRadius::MAX,
            position_type: PositionType::Relative,
        }
        paper
        BorderColor::all(rim)
        Pickable
        act
        on(on_button)
        Children [
            Portrait
            ImageNode
            Node { height: px(AVATAR - 10.0), display: Display::None }
            @ignore_picking()
            --
            Node {
                position_type: PositionType::Absolute,
                bottom: px(-8),
                height: px(15),
                align_items: AlignItems::Center,
                padding: {UiRect::horizontal(px(5))},
                border: px(1),
                border_radius: BorderRadius::all(px(8)),
            }
            BackgroundColor(ro_dark::TOOLTIP_BODY)
            BorderColor::all(ro_dark::PRIMARY_STROKE)
            @ignore_picking()
            Children [ @chrome_text(level.to_string(), 9.0, ro_dark::BRONZE_LIGHT) ]
        ]
    }
}

/// A thin bar `height` px high showing `fill`, with `readout` on its right.
fn track(fill: impl Scene, readout: impl Scene, height: f32) -> impl Scene {
    bsn! {
        Node { align_items: AlignItems::Center, column_gap: px(6) }
        @ignore_picking()
        Children [
            Node {
                flex_grow: 1.0,
                height: px(height),
                border_radius: BorderRadius::all(px(3)),
                overflow: {Overflow::clip()},
            }
            BackgroundColor({Color::BLACK.with_alpha(0.55)})
            @ignore_picking()
            Children [
                @fill
                Node { height: percent(100), border_radius: BorderRadius::all(px(3)) }
                @ignore_picking()
            ]
            --
            Node { width: px(44), flex_shrink: 0.0, justify_content: JustifyContent::FlexEnd }
            @ignore_picking()
            Children [ @readout ]
        ]
    }
}

/// A [`track`] whose fill and percent [`sync_vitals`](super::sync_vitals) writes.
fn vital(fill: Vital, text: Vital, color: Color, ink: Color, height: f32) -> impl Scene {
    track(
        bsn! {
            fill
            Node { width: percent(0) }
            BackgroundColor(color)
        },
        bsn! {
            text
            @chrome_text(String::new(), 9.0, ink)
        },
        height,
    )
}

/// An order button with its key in the corner, lit while it is the standing order.
fn order(icon: &'static str, key: String, act: Act, lit: bool, live: bool) -> impl Scene {
    let rest = match (live, lit) {
        (false, _) => (ro_dark::SLOT_DISABLED, ro_dark::DISABLED_STROKE),
        (true, true) => (ro_dark::TAB_ACTIVE, ro_dark::SELECT_RING),
        (true, false) => (ro_dark::TAB_INACTIVE, ro_dark::MUTED_BRONZE),
    };
    let colors = HoverColors {
        rest,
        hover: (ro_dark::BUTTON, ro_dark::BRONZE_HIGHLIGHT),
    };
    bsn! {
        Node { position_type: PositionType::Relative }
        @ignore_picking()
        Children [
            @plate_button(icon)
            colors
            Node { width: px(30), height: px(30) }
            Pickable { should_block_lower: {live}, is_hoverable: {live} }
            act
            on(on_button)
            --
            @outlined_text(key, 8.0, ro_dark::FAINT)
            Node { position_type: PositionType::Absolute, right: px(2), bottom: px(0) }
        ]
    }
}
