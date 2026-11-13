//! The party frames on the HUD: one compact frame per member, after the
//! `Party Frames (HUD)` artboard. Clicking a frame casts the armed skill on that member.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::ui_widgets::Button;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{chrome_text, ignore_picking};
use lifthrasir_ui::widgets::primitives::{HoverColors, diamond, display_text, ghost_button};

use super::scene::{crown, readout};
use super::view::{FrameView, FramesView, ReadoutPart};
use super::{PartyAction, PartyHost, on_party_button};

const LEFT: f32 = 16.0;
/// NOTE: clears the status frame above at its full size. Anchor the frames to that
/// frame once the HUD has a layout that can stack them.
const TOP: f32 = 236.0;
const WIDTH: f32 = 250.0;

/// Where the frames sit on the HUD. Empty while partyless.
pub(super) fn host() -> impl Scene {
    bsn! {
        PartyHost::Frames
        Node { position_type: PositionType::Absolute, left: px(LEFT), top: px(TOP) }
        @ignore_picking()
    }
}

/// The plate holding the party's name and, unless collapsed, a frame per member.
pub(super) fn panel(view: FramesView) -> impl Scene {
    let (share, tone) = if view.even_share {
        ("Even Share", ro_dark::STATUS_ONLINE)
    } else {
        ("Each Take", ro_dark::MUTED)
    };
    // The chevron art points right; an open list turns it down.
    let turn = UiTransform::from_rotation(Rot2::degrees(if view.collapsed { 0.0 } else { 90.0 }));
    let frames: Vec<_> = view
        .frames
        .into_iter()
        .filter(|_| !view.collapsed)
        .map(frame)
        .collect();
    let studs: Vec<_> = [(true, true), (false, true), (true, false), (false, false)]
        .into_iter()
        .map(stud)
        .collect();
    let fill = ro_dark::WINDOW_BODY.with_alpha(0.82);
    let stroke = ro_dark::BRONZE.with_alpha(0.8);
    let shadow = BoxShadow::new(
        Color::BLACK.with_alpha(0.7),
        Val::ZERO,
        px(4),
        Val::ZERO,
        px(12),
    );
    bsn! {
        Node {
            width: px(WIDTH),
            flex_direction: FlexDirection::Column,
            row_gap: px(5),
            padding: px(7),
            border: px(1),
            border_radius: BorderRadius::all(px(10)),
            position_type: PositionType::Relative,
        }
        BackgroundColor(fill)
        BorderColor::all(stroke)
        shadow
        Pickable
        Children [
            Node { align_items: AlignItems::Center, column_gap: px(6), padding: {UiRect::horizontal(px(2))} }
            @ignore_picking()
            Children [
                @display_text(view.name, 13.0, ro_dark::TITLE)
                --
                @chrome_text(view.online, 11.0, ro_dark::MUTED)
                Node { flex_grow: 1.0 }
                --
                @chrome_text(share.into(), 10.0, tone)
                --
                @ghost_button("chev")
                turn
                PartyAction::ToggleFrames
                on(on_party_button)
            ]
            --
            {frames}
            --
            {studs}
        ]
    }
}

/// A bronze stud on one corner of the plate.
fn stud((left, top): (bool, bool)) -> impl Scene {
    let edge = |near: bool| if near { px(-4) } else { Val::Auto };
    bsn! {
        @diamond(7.0, ro_dark::BRONZE, ro_dark::BRONZE_SHADOW)
        Node {
            position_type: PositionType::Absolute,
            left: {edge(left)},
            right: {edge(!left)},
            top: {edge(top)},
            bottom: {edge(!top)},
        }
    }
}

fn frame(view: FrameView) -> impl Scene {
    let char_id = view.char_id;
    let ink = match (view.online, view.me, view.far) {
        (false, ..) => ro_dark::MUTED,
        (true, true, _) => ro_dark::FROST,
        (true, false, true) => ro_dark::LABEL,
        (true, false, false) => ro_dark::STRONG,
    };
    let stroke = if view.low {
        ro_dark::STATUS_FULL.with_alpha(0.8)
    } else {
        ro_dark::SLOT_STROKE.with_alpha(0.6)
    };
    let fill = ro_dark::INSET.with_alpha(if view.online { 0.78 } else { 0.4 });
    let colors = HoverColors {
        rest: (fill, stroke),
        hover: (ro_dark::SLOT_HOVER.with_alpha(0.9), ro_dark::BUTTON_STROKE),
    };
    let crown = view.leader.then(|| EntityScene(crown(7.0)));
    let tag = readout(char_id, ReadoutPart::Tag);
    let tag = view
        .online
        .then(|| EntityScene(bsn! { tag @chrome_text(String::new(), 10.0, ro_dark::LABEL) }));
    let offline =
        (!view.online).then(|| EntityScene(chrome_text("Offline".into(), 10.0, ro_dark::MUTED)));
    let vitals = view.online.then(|| EntityScene(vitals(char_id)));
    // An offline member cannot be cast on, so their frame is not a button.
    let wired = view.online.then(|| {
        bsn! {
            Button
            Hovered
            colors
            PartyAction::CastOn(char_id)
            Pickable
            on(on_party_button)
        }
    });
    let inert = (!view.online).then(ignore_picking);
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            padding: {UiRect::axes(px(9), px(6))},
            border: px(1),
            border_radius: BorderRadius::all(px(5)),
        }
        BackgroundColor(fill)
        BorderColor::all(stroke)
        @{wired}
        @{inert}
        Children [
            Node { align_items: AlignItems::Center, column_gap: px(6) }
            @ignore_picking()
            Children [
                {crown}
                --
                @chrome_text(view.name, 12.0, ink)
                --
                @chrome_text(view.job, 10.0, ro_dark::MUTED)
                TextLayout { linebreak: LineBreak::NoWrap }
                Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0), overflow: {Overflow::clip()} }
                --
                {tag}
                --
                {offline}
            ]
            --
            {vitals}
        ]
    }
}

/// HP over SP, then where the member is.
fn vitals(char_id: u32) -> impl Scene {
    let place = readout(char_id, ReadoutPart::FrameWhere);
    bsn! {
        Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Stretch }
        @ignore_picking()
        Children [
            @gauge(char_id, ReadoutPart::HpFill, ro_dark::HP, 8.0)
            Node { margin: {UiRect::top(px(5))}, border: px(1) }
            BorderColor::all(ro_dark::BAR_FRAME)
            --
            @gauge(char_id, ReadoutPart::SpFill, ro_dark::SP, 4.0)
            Node { margin: {UiRect::top(px(3))} }
            --
            Node { justify_content: JustifyContent::FlexEnd, margin: {UiRect::top(px(4))} }
            @ignore_picking()
            Children [
                place
                @chrome_text(String::new(), 10.0, ro_dark::MUTED)
            ]
        ]
    }
}

/// A bare resource bar `height` px high.
fn gauge(char_id: u32, part: ReadoutPart, color: Color, height: f32) -> impl Scene {
    let fill = readout(char_id, part);
    bsn! {
        Node { height: px(height), border_radius: BorderRadius::all(px(height / 2.0)) }
        BackgroundColor(ro_dark::BAR_TROUGH)
        @ignore_picking()
        Children [
            fill
            Node {
                width: percent(0),
                height: percent(100),
                border_radius: BorderRadius::all(px(height / 2.0)),
            }
            BackgroundColor(color)
            @ignore_picking()
        ]
    }
}
