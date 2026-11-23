//! The Quest Ledger scenes: the frame spawned once, and what [`rebuild`]
//! (super::rebuild) projects into its hosts. Sizes follow the `Quest` artboard of the
//! Lifthrasir Widgets design.

use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::ui_widgets::{ControlOrientation, ScrollArea};
use bevy_feathers::controls::FeathersScrollbar;
use lifthrasir_ui::rich_text::parse_color_codes;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{chrome_text, drag_window, glyph_icon, ignore_picking};
use lifthrasir_ui::widgets::primitives::{
    ButtonArt, WINDOW_SHADOW, close_button, diamond, display_text, ghost_button_tinted, inset,
    parchment, tab, tab_text, window_title_bar_led, window_titled,
};

use super::view::{Chip, DetailView, GroupView, RowView, StepView, TabView, ends};
use super::{
    Act, Host, LedgerTab, QuestLedgerTitlebar, QuestLedgerWindow, QuestSearch, TimerText,
    on_button, on_click,
};
use crate::screens::shared::{
    bar, chip, colored_lines, overline, row_button, search_field, usable, window_toast,
};

const WINDOW_LEFT: f32 = 48.0;
const WINDOW_TOP: f32 = 48.0;
const WINDOW_WIDTH: f32 = 1024.0;
const LIST_WIDTH: f32 = 360.0;
const LIST_HEIGHT: f32 = 580.0;
const PAGE_HEIGHT: f32 = 600.0;

pub(super) fn window() -> impl Scene {
    bsn! {
        QuestLedgerWindow
        @window_titled()
        Node {
            position_type: PositionType::Absolute,
            left: px(WINDOW_LEFT),
            top: px(WINDOW_TOP),
            width: px(WINDOW_WIDTH + 2.0 * WINDOW_SHADOW),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
            display: Display::None,
        }
        GlobalZIndex(40)
        Pickable
        Children [
            QuestLedgerTitlebar
            @window_title_bar_led(
                "Quest Ledger",
                bsn! { Host::Lead Node {} @ignore_picking() },
                bsn! { @close_button() Act::Close on(on_button) },
            )
            Pickable
            on(drag_window::<QuestLedgerTitlebar, QuestLedgerWindow>)
            --
            Node {
                column_gap: px(22),
                align_items: AlignItems::FlexStart,
                margin: {UiRect::new(px(0), px(0), px(10), px(8))},
            }
            @ignore_picking()
            Children [ @ledger() -- @page() ]
            --
            Host::Toast
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                bottom: px(WINDOW_SHADOW + 22.0),
                justify_content: JustifyContent::Center,
            }
            @ignore_picking()
        ]
    }
}

/// The tabs over the search box, the scrolling list and the pin note.
fn ledger() -> impl Scene {
    bsn! {
        Node {
            width: px(LIST_WIDTH),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
        }
        @ignore_picking()
        Children [
            Host::Tabs
            Node {}
            @ignore_picking()
            --
            @inset()
            Node {
                height: px(LIST_HEIGHT),
                flex_direction: FlexDirection::Column,
                border_radius: {BorderRadius::bottom(px(6))},
            }
            @ignore_picking()
            Children [
                Node { padding: {UiRect::new(px(10), px(10), px(10), px(6))} }
                @ignore_picking()
                Children [ @search_field::<QuestSearch>("Search quests, NPCs or maps") ]
                --
                Node { flex_grow: 1.0, min_height: px(0), position_type: PositionType::Relative }
                @ignore_picking()
                Children [
                    #list
                    Host::List
                    ScrollArea
                    Pickable
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(0), top: px(0), right: px(0), bottom: px(0),
                        flex_direction: FlexDirection::Column,
                        overflow: {Overflow::scroll_y()},
                    }
                    --
                    @FeathersScrollbar { @target: #list, @orientation: {ControlOrientation::Vertical} }
                    Node {
                        position_type: PositionType::Absolute,
                        right: px(2),
                        top: px(4),
                        bottom: px(4),
                        width: px(6),
                    }
                ]
                --
                Node {
                    align_items: AlignItems::Center,
                    column_gap: px(6),
                    padding: {UiRect::axes(px(12), px(8))},
                    border: {UiRect::top(px(1))},
                }
                BorderColor::all(ro_dark::INSET_RULE)
                @ignore_picking()
                Children [
                    @glyph_icon("pin", 12.0, ro_dark::FROST_GLOW)
                    --
                    Host::Note
                    Node {}
                    @ignore_picking()
                ]
            ]
        ]
    }
}

/// The parchment page: the picked quest over its footer.
fn page() -> impl Scene {
    bsn! {
        Node {
            flex_grow: 1.0,
            min_width: px(0),
            padding: {UiRect::top(px(4))},
            flex_direction: FlexDirection::Column,
        }
        @ignore_picking()
        Children [
            @parchment()
            Node {
                height: px(PAGE_HEIGHT),
                flex_direction: FlexDirection::Column,
                border_radius: BorderRadius::all(px(6)),
                overflow: Overflow::clip(),
            }
            @ignore_picking()
            Children [
                Node { flex_grow: 1.0, min_height: px(0), position_type: PositionType::Relative }
                @ignore_picking()
                Children [
                    #detail
                    Host::Detail
                    ScrollArea
                    Pickable
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(0), top: px(0), right: px(0), bottom: px(0),
                        flex_direction: FlexDirection::Column,
                        padding: {UiRect::new(px(26), px(26), px(22), px(12))},
                        overflow: {Overflow::scroll_y()},
                    }
                    --
                    @FeathersScrollbar { @target: #detail, @orientation: {ControlOrientation::Vertical} }
                    Node {
                        position_type: PositionType::Absolute,
                        right: px(4),
                        top: px(8),
                        bottom: px(8),
                        width: px(6),
                    }
                ]
                --
                Host::Footer
                Node { flex_shrink: 0.0, flex_direction: FlexDirection::Column }
                @ignore_picking()
            ]
        ]
    }
}

pub(super) fn lead(text: String) -> impl Scene {
    chrome_text(text, 12.0, ro_dark::LABEL)
}

pub(super) fn note(text: String) -> impl Scene {
    chrome_text(text, 11.0, ro_dark::MUTED)
}

pub(super) fn toast(text: String) -> impl Scene {
    window_toast(text, true)
}

fn tab_button(view: TabView) -> impl Scene {
    let (ink, count_ink) = tab_text(view.selected);
    let act = Act::Tab(view.tab);
    bsn! {
        @tab(view.selected)
        act
        on(on_click)
        Children [
            @chrome_text(view.label.into(), 12.0, ink)
            --
            @chrome_text(view.count.to_string(), 11.0, count_ink)
        ]
    }
}

pub(super) fn tabs(tabs: Vec<TabView>) -> impl Scene {
    let tabs: Vec<_> = tabs.into_iter().map(tab_button).collect();
    bsn! {
        Node { width: px(LIST_WIDTH), column_gap: px(4) }
        @ignore_picking()
        Children [ {tabs} ]
    }
}

/// The groups of rows, or what an empty tab says.
pub(super) fn list(groups: Vec<GroupView>, tab: LedgerTab) -> Box<dyn Scene> {
    if groups.is_empty() {
        let text = match tab {
            LedgerTab::Shelved => {
                "Shelve a quest to hide it from the screen without losing progress"
            }
            _ => "Nothing here yet",
        };
        return Box::new(bsn! {
            Node {
                margin: {UiRect::new(px(16), px(16), px(120), px(0))},
                justify_content: JustifyContent::Center,
            }
            @ignore_picking()
            Children [ @chrome_text(text.into(), 12.0, ro_dark::MUTED) ]
        });
    }
    let active = tab == LedgerTab::Active;
    let groups: Vec<_> = groups.into_iter().map(|g| group(g, active)).collect();
    Box::new(bsn! {
        Node { flex_direction: FlexDirection::Column, flex_shrink: 0.0 }
        @ignore_picking()
        Children [ {groups} ]
    })
}

fn group(view: GroupView, active: bool) -> impl Scene {
    let head = view.label.map(|label| {
        let tone = if label == "READY TO TURN IN" {
            ro_dark::STATUS_ONLINE
        } else {
            ro_dark::BASE_EXP
        };
        EntityScene(group_head(label, tone, view.rows.len()))
    });
    let rows: Vec<_> = view.rows.into_iter().map(|r| row(r, active)).collect();
    bsn! {
        Node { flex_direction: FlexDirection::Column }
        @ignore_picking()
        Children [ {head} -- {rows} ]
    }
}

fn group_head(label: &'static str, tone: Color, count: usize) -> impl Scene {
    bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: px(8),
            padding: {UiRect::new(px(12), px(12), px(10), px(4))},
        }
        @ignore_picking()
        Children [
            @diamond(7.0, tone, Color::NONE)
            --
            @overline(label.into(), ro_dark::ACCENT)
            --
            @chrome_text(count.to_string(), 10.0, ro_dark::MUTED)
        ]
    }
}

/// One quest: a clickable row, and on the Active tab its progress bar and pin.
fn row(view: RowView, active: bool) -> impl Scene {
    let title_ink = match (view.ready, view.selected) {
        (true, _) => ro_dark::STATUS_ONLINE,
        (false, true) => ro_dark::STRONG,
        (false, false) => ro_dark::BODY,
    };
    let fill = match (view.selected, view.ready) {
        (true, _) => ro_dark::FROST_GLOW.with_alpha(0.08),
        (false, true) => ro_dark::STATUS_ONLINE.with_alpha(0.07),
        (false, false) => Color::NONE,
    };
    let edge = if view.selected { 3.0 } else { 0.0 };
    let chip = view.chip.map(|c| EntityScene(row_chip(view.id, c)));
    let bar = active.then(|| {
        let tone = if view.ready {
            ro_dark::JOB_EXP
        } else {
            ro_dark::BASE_EXP
        };
        EntityScene(bsn! {
            @bar(f32::from(view.bar_pct), tone, false)
            Node { margin: {UiRect::top(px(6))} }
        })
    });
    let pin = active.then(|| EntityScene(pin_button(view.id, view.pinned)));
    let act = Act::Select(view.id);
    bsn! {
        Node { position_type: PositionType::Relative, flex_direction: FlexDirection::Column }
        @ignore_picking()
        Children [
            act
            Node {
                flex_direction: FlexDirection::Column,
                padding: {UiRect::new(px(12.0 - edge), px(40), px(9), px(9))},
                border: {UiRect { left: px(edge), bottom: px(1), ..default() }},
            }
            BackgroundColor(fill)
            BorderColor { left: ro_dark::SELECT_RING, bottom: ro_dark::INSET_RULE }
            Pickable
            on(on_click)
            Children [
                @chrome_text(view.title, 13.0, title_ink)
                --
                Node { margin: {UiRect::top(px(2))}, align_items: AlignItems::Center, column_gap: px(6) }
                @ignore_picking()
                Children [ @chrome_text(view.sub_line, 11.0, ro_dark::MUTED) -- {chip} ]
                --
                {bar}
            ]
            --
            {pin}
        ]
    }
}

fn row_chip(id: u32, view: Chip) -> Box<dyn Scene> {
    match view {
        Chip::Ready => Box::new(chip(
            "Ready to turn in".into(),
            ro_dark::STATUS_ONLINE.with_alpha(0.18),
            ro_dark::STATUS_ONLINE,
        )),
        Chip::Ends(text) => Box::new(bsn! {
            @chip(text, ro_dark::STATUS_BUSY.with_alpha(0.16), ro_dark::STATUS_BUSY)
            TimerText(id)
        }),
    }
}

fn pin_button(id: u32, pinned: bool) -> impl Scene {
    let tint = if pinned {
        ro_dark::FROST_GLOW
    } else {
        ro_dark::FAINT
    };
    let act = Act::Pin(id);
    bsn! {
        @ghost_button_tinted("pin", tint)
        Node { position_type: PositionType::Absolute, right: px(8), top: px(9) }
        act
        on(on_button)
    }
}

/// What the parchment says while no quest is picked.
pub(super) fn pick() -> impl Scene {
    bsn! {
        Node {
            flex_grow: 1.0,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        @ignore_picking()
        Children [
            @chrome_text("Pick a quest from the ledger".into(), 13.0, ro_dark::BRONZE_SHADOW)
        ]
    }
}

/// The picked quest written on the parchment: title, giver and deadline, story and
/// objectives.
pub(super) fn detail(view: DetailView) -> impl Scene {
    let giver = view
        .giver_line
        .map(|line| EntityScene(chrome_text(line, 12.0, ro_dark::BRONZE_SHADOW)));
    let timer = view
        .timer
        .map(|left| EntityScene(deadline(view.id, ends(left))));
    let story: Vec<_> = view
        .story
        .iter()
        .map(|line| parse_color_codes(line, ro_dark::INK))
        .collect();
    let steps: Vec<_> = view.steps.into_iter().map(step).collect();
    bsn! {
        Node { flex_direction: FlexDirection::Column, flex_shrink: 0.0 }
        @ignore_picking()
        Children [
            @display_text(view.title, 24.0, ro_dark::INK)
            --
            Node {
                margin: {UiRect::top(px(8))},
                align_items: AlignItems::Center,
                column_gap: px(8),
                flex_wrap: FlexWrap::Wrap,
            }
            @ignore_picking()
            Children [ {giver} -- {timer} ]
            --
            @colored_lines(story)
            Node { margin: {UiRect::top(px(12))} }
            --
            @rule()
            --
            @overline("OBJECTIVES".into(), ro_dark::BRONZE_SHADOW)
            --
            Node { margin: {UiRect::top(px(8))}, flex_direction: FlexDirection::Column, row_gap: px(10) }
            @ignore_picking()
            Children [ {steps} ]
        ]
    }
}

fn deadline(id: u32, text: String) -> impl Scene {
    bsn! {
        TimerText(id)
        Node {}
        @ignore_picking()
        Children [ @chrome_text(text, 12.0, ro_dark::CLOSE_HOVER) ]
    }
}

/// A line, a diamond and a line across the page.
fn rule() -> impl Scene {
    let line = ro_dark::BRONZE_SHADOW.with_alpha(0.45);
    bsn! {
        Node {
            margin: {UiRect::new(px(0), px(0), px(14), px(10))},
            align_items: AlignItems::Center,
            column_gap: px(8),
        }
        @ignore_picking()
        Children [
            Node { flex_grow: 1.0, height: px(1) }
            BackgroundColor(line)
            --
            @diamond(6.0, ro_dark::BRONZE_SHADOW.with_alpha(0.7), Color::NONE)
            --
            Node { flex_grow: 1.0, height: px(1) }
            BackgroundColor(line)
        ]
    }
}

/// An objective: a ticked box, its label and count, and a bar when it counts.
fn step(view: StepView) -> impl Scene {
    let (ink, count_ink) = if view.done {
        (ro_dark::BRONZE_SHADOW, ro_dark::JOB_EXP)
    } else {
        (ro_dark::INK, ro_dark::INK)
    };
    let bar = (!view.count_text.is_empty()).then(|| {
        let tone = if view.done {
            ro_dark::JOB_EXP
        } else {
            ro_dark::BRONZE_SHADOW
        };
        EntityScene(bsn! {
            @bar(f32::from(view.bar_pct), tone, false)
            Node { margin: {UiRect::top(px(5))} }
        })
    });
    bsn! {
        Node { column_gap: px(10), align_items: AlignItems::FlexStart }
        @ignore_picking()
        Children [
            @tick_box(view.done)
            --
            Node { flex_grow: 1.0, min_width: px(0), flex_direction: FlexDirection::Column }
            @ignore_picking()
            Children [
                Node { align_items: AlignItems::Baseline, column_gap: px(8) }
                @ignore_picking()
                Children [
                    @chrome_text(view.label, 13.0, ink)
                    Node { flex_grow: 1.0 }
                    --
                    @chrome_text(view.count_text, 12.0, count_ink)
                ]
                --
                {bar}
            ]
        ]
    }
}

/// The ink square before an objective, filled and ticked once it is done.
fn tick_box(done: bool) -> impl Scene {
    let fill = if done {
        ro_dark::BRONZE_SHADOW
    } else {
        Color::NONE
    };
    let tick = done.then(|| EntityScene(glyph_icon("check", 10.0, ro_dark::HERO)));
    bsn! {
        Node {
            width: px(16),
            height: px(16),
            flex_shrink: 0.0,
            margin: {UiRect::top(px(1))},
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: px(1),
            border_radius: BorderRadius::all(px(2)),
        }
        BackgroundColor(fill)
        BorderColor::all(ro_dark::BRONZE_SHADOW)
        @ignore_picking()
        Children [ {tick} ]
    }
}

/// Navigate and Shelve or Resume, with the ready line pushed to the right.
pub(super) fn footer(
    navigate: bool,
    shelve: Option<&'static str>,
    ready: Option<String>,
) -> impl Scene {
    let shelve = shelve.map(|label| {
        let act = if label == "Resume" {
            Act::Resume
        } else {
            Act::Shelve
        };
        EntityScene(bsn! {
            @row_button(ButtonArt::NORMAL, label, 72.0)
            act
            on(on_button)
        })
    });
    let ready = ready.map(|text| EntityScene(chrome_text(text, 12.0, ro_dark::JOB_EXP)));
    bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: px(8),
            padding: {UiRect::new(px(26), px(26), px(12), px(16))},
            border: {UiRect::top(px(1))},
        }
        BackgroundColor({ro_dark::PARCHMENT_SHADE.with_alpha(0.35)})
        BorderColor::all(ro_dark::BRONZE_SHADOW.with_alpha(0.35))
        @ignore_picking()
        Children [
            @row_button(ButtonArt::NORMAL, "Navigate", 84.0)
            @usable(navigate, Color::WHITE)
            Act::Navigate
            on(on_button)
            --
            {shelve}
            --
            Node { flex_grow: 1.0 }
            @ignore_picking()
            --
            {ready}
        ]
    }
}
