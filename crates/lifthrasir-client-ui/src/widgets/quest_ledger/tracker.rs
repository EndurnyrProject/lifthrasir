//! The quest tracker on the HUD and its objective banner, after the `QuestTracker`
//! artboard of the Lifthrasir Widgets design: the pinned quests with their objectives
//! under a collapsible header, and a line across the top when an objective is done.

use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::ui_widgets::Button;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::ignore_picking;
use lifthrasir_ui::widgets::primitives::{diamond, display_text, ghost_button};

use super::view::{StepView, TrackerQuest};
use super::{Act, Host, TimerText, on_button};
use crate::screens::shared::{outlined_text, overline};
use crate::widgets::hud_layout::{HudWidget, movable};

/// NOTE: a guess, not checked on screen: the right edge under the minimap (16 px
/// margin, 180 px frame and its coordinate readout). Anchor it to the minimap once the
/// HUD has a layout that can stack its widgets.
const RIGHT: f32 = 16.0;
const TOP: f32 = 236.0;
const WIDTH: f32 = 280.0;
const BANNER_TOP: f32 = 28.0;

const TITLE_INK: Color = Color::srgb_u8(0xe8, 0xd8, 0xb0);
const BANNER_INK: Color = Color::srgb_u8(0xf0, 0xe4, 0xc4);

/// The movable plate: its header and the list under it.
pub(super) fn plate() -> impl Scene {
    bsn! {
        @movable(HudWidget::QuestTracker)
        Node {
            position_type: PositionType::Absolute,
            right: px(RIGHT),
            top: px(TOP),
            width: px(WIDTH),
            flex_direction: FlexDirection::Column,
        }
        @ignore_picking()
        Children [
            Host::TrackerHead
            Node {}
            @ignore_picking()
            --
            Host::TrackerList
            Node { margin: {UiRect::top(px(6))}, flex_direction: FlexDirection::Column, row_gap: px(4) }
            @ignore_picking()
        ]
    }
}

/// Where the banner appears: across the top of the screen.
pub(super) fn banner_host() -> impl Scene {
    bsn! {
        Host::Banner
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(BANNER_TOP),
            justify_content: JustifyContent::Center,
        }
        @ignore_picking()
    }
}

pub(super) fn banner(text: String) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center }
        @ignore_picking()
        Children [
            @overline("OBJECTIVE COMPLETE".into(), ro_dark::STATUS_ONLINE)
            --
            @display_text(text, 20.0, BANNER_INK)
        ]
    }
}

/// The title, how many quests are tracked and the chevron. The header is what the
/// plate is dragged by.
pub(super) fn head(count: usize, collapsed: bool) -> impl Scene {
    let tilt = UiTransform::from_rotation(Rot2::degrees(if collapsed { 0.0 } else { 90.0 }));
    bsn! {
        Node {
            width: percent(100),
            align_items: AlignItems::Center,
            column_gap: px(6),
            padding: {UiRect::new(px(4), px(4), px(0), px(6))},
            border: {UiRect::bottom(px(1))},
        }
        BorderColor::all(ro_dark::BRONZE.with_alpha(0.6))
        Pickable
        Children [
            @display_text("Quests".into(), 13.0, TITLE_INK)
            --
            @outlined_text(format!("{count} tracked"), 11.0, ro_dark::LABEL)
            --
            Node { flex_grow: 1.0 }
            @ignore_picking()
            --
            @ghost_button("chev")
            tilt
            Act::ToggleTracker
            on(on_button)
        ]
    }
}

pub(super) fn list(quests: Vec<TrackerQuest>) -> impl Scene {
    let rows: Vec<_> = quests.into_iter().map(row).collect();
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(4) }
        @ignore_picking()
        Children [ {rows} ]
    }
}

/// One pinned quest: a button that opens it in the ledger.
fn row(quest: TrackerQuest) -> impl Scene {
    let ready = quest.ready_text.is_some();
    let (fill, stroke, ink) = if ready {
        (
            ro_dark::STATUS_ONLINE.with_alpha(0.1),
            ro_dark::STATUS_ONLINE.with_alpha(0.45),
            ro_dark::STATUS_ONLINE,
        )
    } else {
        (Color::NONE, Color::NONE, ro_dark::STRONG)
    };
    let tone = if ready {
        ro_dark::STATUS_ONLINE
    } else {
        ro_dark::STATUS_BUSY
    };
    let id = quest.id;
    let timer = quest.timer.map(|left| {
        EntityScene(bsn! {
            TimerText { id, bare: true }
            Node {}
            @ignore_picking()
            Children [ @outlined_text(left, 10.0, ro_dark::STATUS_BUSY) ]
        })
    });
    let body: Vec<Box<dyn Scene>> = match quest.ready_text {
        Some(text) => vec![Box::new(outlined_text(text, 12.0, ro_dark::STATUS_ONLINE))],
        None => quest.steps.into_iter().map(step).collect(),
    };
    let act = Act::Open(id);
    bsn! {
        Button
        act
        Node {
            flex_direction: FlexDirection::Column,
            padding: {UiRect::axes(px(10), px(8))},
            border: px(1),
            border_radius: BorderRadius::all(px(5)),
        }
        BackgroundColor(fill)
        BorderColor::all(stroke)
        Pickable
        on(on_button)
        Children [
            Node { align_items: AlignItems::Center, column_gap: px(7) }
            @ignore_picking()
            Children [
                @diamond(7.0, tone, Color::NONE)
                --
                @outlined_text(quest.title, 13.0, ink)
                --
                Node { flex_grow: 1.0 }
                @ignore_picking()
                --
                {timer}
            ]
            --
            Node { margin: {UiRect::new(px(14), px(0), px(3), px(0))}, flex_direction: FlexDirection::Column, row_gap: px(1) }
            @ignore_picking()
            Children [ {body} ]
        ]
    }
}

/// An objective: a diamond ticked once it is done, its label and its count.
fn step(view: StepView) -> Box<dyn Scene> {
    let (fill, stroke, ink, count_ink) = if view.done {
        (
            ro_dark::STATUS_ONLINE,
            ro_dark::STATUS_ONLINE,
            ro_dark::MUTED,
            ro_dark::STATUS_ONLINE,
        )
    } else {
        (Color::NONE, ro_dark::MUTED, ro_dark::BODY, ro_dark::STRONG)
    };
    let count = view.count_text.replace('/', " / ");
    Box::new(bsn! {
        Node { align_items: AlignItems::Center, column_gap: px(8) }
        @ignore_picking()
        Children [
            @diamond(7.0, fill, stroke)
            --
            @outlined_text(view.label, 12.0, ink)
            --
            Node { flex_grow: 1.0 }
            @ignore_picking()
            --
            @outlined_text(count, 12.0, count_ink)
        ]
    })
}
