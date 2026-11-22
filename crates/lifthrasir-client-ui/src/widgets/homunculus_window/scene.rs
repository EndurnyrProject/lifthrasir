//! The Homunculus window scenes: the frame spawned once, and what [`rebuild`]
//! (super::rebuild) projects into its hosts. Sizes follow the `Homunculus` artboard of
//! the Lifthrasir Widgets design.

use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::ui_widgets::{ControlOrientation, ScrollArea};
use bevy_feathers::controls::FeathersScrollbar;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{chrome_text, drag_window, glyph_icon, ignore_picking};
use lifthrasir_ui::widgets::primitives::{
    BUTTON_SHADOW, ButtonArt, WINDOW_SHADOW, close_button, display_text, ghost_button, inset,
    parchment, segment, tab, tab_text, window_title_bar, window_title_bar_led, window_titled,
};
use net_contract::dto::{HomunculusAiSkillMode as Mode, HomunculusAiStance as Stance};

use super::tactics::MobList;
use super::view::{
    BarView, BodyView, CareView, ConfirmView, Control, DialView, INTIMACY, NameView, Order,
    OrdersView, PageView, PortraitView, RuleView, SkillRow, SkillsView, StatusView, TabsView,
    TacticRow, TacticsBody, TacticsView,
};
use super::{
    Act, CompanionLayout, Cooldown, HomunculusTab, HomunculusTitlebar, HomunculusWindow, Host,
    MobField, NAME_MAX_CHARS, NameField, OwnerMenu, Portrait, SkillIcon, drag_skill, on_button,
    on_click, on_dial_drag, on_dial_press,
};
use crate::screens::shared::{
    DANGER_TINT, RowTone, SliderOf, SliderPart, bar, chip, even_segment, fit_button, flush_button,
    link, menu_row, modal_layer, number_field, outlined_text, overline, row_button, slider, switch,
    text_field, usable, wide_button,
};

const WINDOW_LEFT: f32 = 60.0;
const WINDOW_TOP: f32 = 30.0;
const WINDOW_WIDTH: f32 = 1100.0;
const ASIDE_WIDTH: f32 = 320.0;
const PORTRAIT_HEIGHT: f32 = 210.0;
/// Fixed height of the page under the tabs: it scrolls, so the window never grows
/// with the open tab.
const PANEL_HEIGHT: f32 = 520.0;
/// Over the HUD's windows and toasts, under the system dialog.
const CONFIRM_Z: i32 = 1450;

pub(super) fn window() -> impl Scene {
    bsn! {
        HomunculusWindow
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
            HomunculusTitlebar
            @window_title_bar_led(
                "Homunculus",
                bsn! { Host::Lead Node {} @ignore_picking() },
                title_buttons(),
            )
            Pickable
            on(drag_window::<HomunculusTitlebar, HomunculusWindow>)
            --
            Host::Body
            Node {}
            @ignore_picking()
            --
            @companion()
        ]
    }
}

fn title_buttons() -> impl Scene {
    bsn! {
        Node { align_items: AlignItems::Center, column_gap: px(8) }
        @ignore_picking()
        Children [
            @ghost_button("refresh")
            Act::Refresh
            on(on_button)
            --
            @close_button()
            Act::Close
            on(on_button)
        ]
    }
}

pub(super) fn lead(text: String, tone: Color) -> impl Scene {
    chrome_text(text, 12.0, tone)
}

/// What fills the window while there is no companion to lay out.
pub(super) fn body(view: BodyView) -> impl Scene {
    let (title, text) = match view {
        BodyView::Loading => ("Loading companion\u{2026}", ""),
        BodyView::Absent => (
            "No homunculus",
            "Craft an Embryo, then use Call Homunculus to bring a new companion to life.",
        ),
    };
    bsn! {
        Node {
            flex_grow: 1.0,
            height: px(PANEL_HEIGHT + 50.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: px(6),
        }
        @ignore_picking()
        Children [
            @display_text(title.into(), 20.0, ro_dark::HERO)
            --
            @chrome_text(text.into(), 13.0, ro_dark::MUTED)
        ]
    }
}

/// The companion on the left, the tabbed pages on the right, shown while there is a
/// companion. Every region is a host, so a name being typed or a scrolled page
/// outlives the other regions' changes.
fn companion() -> impl Scene {
    bsn! {
        CompanionLayout
        Node {
            column_gap: px(18),
            margin: {UiRect::new(px(0), px(0), px(10), px(8))},
            display: Display::None,
        }
        @ignore_picking()
        Children [
            Node {
                width: px(ASIDE_WIDTH),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                row_gap: px(12),
            }
            @ignore_picking()
            Children [
                Host::Portrait
                Node { height: px(PORTRAIT_HEIGHT) }
                @ignore_picking()
                --
                Host::Name
                Node { flex_direction: FlexDirection::Column }
                @ignore_picking()
                --
                Host::Care
                Node { flex_direction: FlexDirection::Column }
                @ignore_picking()
                --
                @overline("ORDERS".into(), ro_dark::ACCENT)
                --
                Host::Orders
                Node { flex_direction: FlexDirection::Column }
                @ignore_picking()
            ]
            --
            Node {
                flex_grow: 1.0,
                flex_basis: px(0),
                min_width: px(0),
                flex_direction: FlexDirection::Column,
            }
            @ignore_picking()
            Children [
                Host::Tabs
                Node { column_gap: px(4) }
                @ignore_picking()
                --
                @inset()
                Node { height: px(PANEL_HEIGHT), position_type: PositionType::Relative }
                @ignore_picking()
                Children [
                    #page
                    Host::Page
                    ScrollArea
                    Pickable
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(0), top: px(0), right: px(0), bottom: px(0),
                        flex_direction: FlexDirection::Column,
                        padding: {UiRect::new(px(18), px(22), px(16), px(16))},
                        overflow: {Overflow::scroll_y()},
                    }
                    --
                    @FeathersScrollbar { @target: #page, @orientation: {ControlOrientation::Vertical} }
                    Node {
                        position_type: PositionType::Absolute,
                        right: px(4),
                        top: px(8),
                        bottom: px(8),
                        width: px(6),
                    }
                ]
            ]
        ]
    }
}

/// A dark pill on the parchment, with a dot in its tone when `dotted`.
fn tag(text: String, tone: Color, dotted: bool) -> impl Scene {
    let dot = dotted.then(|| {
        EntityScene(bsn! {
            Node { width: px(7), height: px(7), border_radius: BorderRadius::MAX }
            BackgroundColor(tone)
            @ignore_picking()
        })
    });
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            top: px(10),
            height: px(22),
            align_items: AlignItems::Center,
            column_gap: px(6),
            padding: {UiRect::horizontal(px(9))},
            border_radius: BorderRadius::all(px(11)),
        }
        BackgroundColor({ro_dark::TOOLTIP_BODY.with_alpha(0.85)})
        @ignore_picking()
        Children [ {dot} -- @chrome_text(text, 11.0, tone) ]
    }
}

pub(super) fn portrait(view: PortraitView) -> impl Scene {
    let tint = Color::WHITE.with_alpha(if view.away { 0.45 } else { 1.0 });
    bsn! {
        @parchment()
        Node {
            flex_grow: 1.0,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            position_type: PositionType::Relative,
            border_radius: BorderRadius::all(px(6)),
        }
        @ignore_picking()
        Children [
            Portrait
            ImageNode { color: {tint} }
            Node { height: px(150), display: Display::None }
            @ignore_picking()
            --
            @tag(view.activity.into(), view.tone, true)
            Node { left: px(10) }
            --
            @tag(format!("Lv {}", view.level), ro_dark::BRONZE_LIGHT, false)
            Node { right: px(10) }
        ]
    }
}

pub(super) fn name(view: NameView) -> Box<dyn Scene> {
    if view.renaming {
        return Box::new(rename_form(view.name));
    }
    let rename = view.can_rename.then(|| {
        EntityScene(bsn! {
            @link("Rename")
            Act::StartRename
            on(on_click)
        })
    });
    Box::new(bsn! {
        Node { align_items: AlignItems::Baseline, column_gap: px(8) }
        @ignore_picking()
        Children [
            @display_text(view.name, 22.0, ro_dark::HERO)
            --
            @chrome_text(view.species, 12.0, ro_dark::MUTED)
            Node { flex_grow: 1.0 }
            --
            {rename}
        ]
    })
}

fn rename_form(name: String) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(4) }
        @ignore_picking()
        Children [
            Node { align_items: AlignItems::Center, column_gap: px(6) }
            @ignore_picking()
            Children [
                NameField
                @text_field(name, NAME_MAX_CHARS)
                Node {
                    flex_grow: 1.0,
                    flex_basis: px(0),
                    min_width: px(0),
                    height: px(28),
                    padding: {UiRect::axes(px(8), px(4))},
                }
                --
                @row_button(ButtonArt::PRIMARY, "Save", 48.0)
                Act::SaveName
                on(on_button)
                --
                @row_button(ButtonArt::NORMAL, "Cancel", 56.0)
                Act::CancelName
                on(on_button)
            ]
            --
            @chrome_text(
                "You can rename a homunculus only once".into(),
                11.0,
                ro_dark::BRONZE_LIGHT,
            )
        ]
    }
}

fn mark((at, color): (f32, Color)) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: {Val::Percent(at)},
            top: px(0),
            bottom: px(0),
            width: px(2),
        }
        BackgroundColor(color)
        @ignore_picking()
    }
}

/// A [`bar`] with lines across it at the given percents.
fn marked_bar(fill: f32, color: Color, marks: Vec<(f32, Color)>) -> impl Scene {
    let marks: Vec<_> = marks.into_iter().map(mark).collect();
    bsn! {
        Node { flex_direction: FlexDirection::Column, position_type: PositionType::Relative }
        @ignore_picking()
        Children [ @bar(fill, color, false) -- {marks} ]
    }
}

/// A caption on the left, a word in its tone and an amount on the right.
fn gauge_head(label: &'static str, word: String, tone: Color, amount: String) -> impl Scene {
    bsn! {
        Node { align_items: AlignItems::Baseline, column_gap: px(4) }
        @ignore_picking()
        Children [
            @overline(label.into(), ro_dark::ACCENT)
            Node { flex_grow: 1.0 }
            --
            @chrome_text(word, 11.0, tone)
            --
            @chrome_text(amount, 11.0, ro_dark::FAINT)
        ]
    }
}

pub(super) fn care(view: CareView) -> impl Scene {
    let grade_ticks = [25.0, 50.0, 75.0, 91.0]
        .map(|at| (at, Color::BLACK.with_alpha(0.6)))
        .to_vec();
    let feed_line = view
        .feed_line
        .map(|at| (at, ro_dark::FROST_GLOW))
        .into_iter()
        .collect();
    bsn! {
        Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, row_gap: px(4) }
        @ignore_picking()
        Children [
            @gauge_head(
                "INTIMACY",
                view.grade.into(),
                INTIMACY,
                format!("\u{b7} {} / 1,000", view.intimacy),
            )
            --
            @marked_bar(view.intimacy_fill, INTIMACY, grade_ticks)
            --
            @gauge_head(
                "HUNGER",
                view.hunger_word.into(),
                view.hunger_tone,
                format!("\u{b7} {} / 100", view.hunger),
            )
            Node { margin: {UiRect::top(px(5))} }
            --
            @marked_bar(view.hunger as f32, view.hunger_tone, feed_line)
            --
            Node { align_items: AlignItems::Center, column_gap: px(8), margin: {UiRect::top(px(2))} }
            @ignore_picking()
            Children [
                @chrome_text(view.note, 11.0, ro_dark::MUTED)
                Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0) }
                --
                @fit_button(ButtonArt::NORMAL, chrome_text(view.feed, 11.0, ro_dark::BUTTON_LABEL))
                Node { height: px(26.0 + 2.0 * BUTTON_SHADOW) }
                @usable(view.can_feed, Color::WHITE)
                Act::Feed
                on(on_button)
            ]
        ]
    }
}

fn order_segment(label: &'static str, act: Act, lit: bool) -> impl Scene {
    bsn! {
        @even_segment(label, lit)
        Node { height: px(32) }
        act
        on(on_click)
    }
}

pub(super) fn orders(view: OrdersView) -> impl Scene {
    let lit = |order| view.order == Some(order);
    let row: Box<dyn Scene> = if view.out {
        Box::new(bsn! {
            Node { column_gap: px(6) }
            @ignore_picking()
            Children [
                @order_segment("Follow", Act::Follow, lit(Order::Follow))
                --
                @order_segment("Attack", Act::Attack, lit(Order::Attack))
                --
                @order_segment("Stay", Act::Standby, lit(Order::Stay))
                --
                @order_segment("Move", Act::Move, lit(Order::Move))
            ]
        })
    } else {
        Box::new(bsn! {
            Node { height: px(32), align_items: AlignItems::Center }
            @ignore_picking()
            Children [
                @chrome_text(
                    format!("{} takes orders once it is out with you", view.name),
                    12.0,
                    ro_dark::MUTED,
                )
            ]
        })
    };
    bsn! {
        Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, row_gap: px(6) }
        @ignore_picking()
        Children [
            @row
            --
            Node { column_gap: px(6) }
            @ignore_picking()
            Children [
                @flush_button(ButtonArt::NORMAL, "Send to Rest")
                @usable(view.out, Color::WHITE)
                Act::Rest
                on(on_button)
                --
                @wide_button(ButtonArt::NORMAL, "Delete", 72.0)
                @usable(true, DANGER_TINT)
                Act::AskDelete
                on(on_button)
            ]
        ]
    }
}

fn tab_button(
    label: &'static str,
    page: HomunculusTab,
    open: HomunculusTab,
    badge: Option<(String, Color)>,
) -> impl Scene {
    let active = page == open;
    let (ink, _) = tab_text(active);
    let badge = badge.map(|(text, fill)| EntityScene(chip(text, fill, ro_dark::CANVAS)));
    let act = Act::Tab(page);
    bsn! {
        @tab(active)
        Node {
            flex_grow: 0.0,
            flex_basis: Val::Auto,
            height: px(32),
            padding: {UiRect::horizontal(px(16))},
        }
        act
        on(on_click)
        Children [ @chrome_text(label.into(), 13.0, ink) -- {badge} ]
    }
}

pub(super) fn tabs(view: TabsView) -> impl Scene {
    let points = view
        .points
        .map(|points| (points.to_string(), ro_dark::RARE_RING));
    let unsaved = view
        .unsaved
        .then(|| ("\u{2022}".to_owned(), ro_dark::FROST_GLOW));
    bsn! {
        Node { flex_grow: 1.0, column_gap: px(4) }
        @ignore_picking()
        Children [
            @tab_button("Status", HomunculusTab::Status, view.tab, None)
            --
            @tab_button("Skills", HomunculusTab::Skills, view.tab, points)
            --
            @tab_button("Behaviour", HomunculusTab::Tactics, view.tab, unsaved)
        ]
    }
}

pub(super) fn page(view: PageView) -> Box<dyn Scene> {
    match view {
        PageView::Status(view) => Box::new(status_page(view)),
        PageView::Skills(view) => Box::new(skills_page(view)),
        PageView::Tactics(view) => Box::new(tactics_page(view)),
    }
}

/// A page's root: a column that keeps its height inside the scrolling panel.
fn page_column() -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Column, flex_shrink: 0.0 }
        @ignore_picking()
    }
}

fn section(label: String) -> impl Scene {
    bsn! {
        @overline(label, ro_dark::ACCENT)
        Node { margin: {UiRect::new(px(0), px(0), px(20), px(8))} }
    }
}

fn status_bar_cell(view: BarView) -> impl Scene {
    let gauge = view
        .fill
        .map(|fill| EntityScene(bar(fill, view.color, false)));
    bsn! {
        Node {
            flex_grow: 1.0,
            flex_basis: percent(40),
            min_width: px(0),
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
        }
        @ignore_picking()
        Children [
            Node { align_items: AlignItems::Baseline }
            @ignore_picking()
            Children [
                @overline(view.label.into(), ro_dark::ACCENT)
                Node { flex_grow: 1.0 }
                --
                @chrome_text(view.value, 11.0, ro_dark::STRONG)
            ]
            --
            {gauge}
        ]
    }
}

fn attribute((label, value): (&'static str, u32)) -> impl Scene {
    bsn! {
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: {UiRect::vertical(px(7))},
            border_radius: BorderRadius::all(px(5)),
        }
        BackgroundColor(ro_dark::SLOT)
        @ignore_picking()
        Children [
            @overline(label.into(), ro_dark::ACCENT)
            --
            @chrome_text(value.to_string(), 16.0, ro_dark::STRONG)
        ]
    }
}

fn combat_line((label, value): (&'static str, u32)) -> impl Scene {
    bsn! {
        Node {
            flex_grow: 1.0,
            flex_basis: percent(40),
            min_width: px(0),
            padding: {UiRect::axes(px(2), px(7))},
            border: {UiRect::bottom(px(1))},
        }
        BorderColor::all(ro_dark::INSET_RULE)
        @ignore_picking()
        Children [
            @chrome_text(label.into(), 13.0, ro_dark::MUTED)
            Node { flex_grow: 1.0 }
            --
            @chrome_text(value.to_string(), 13.0, ro_dark::STRONG)
        ]
    }
}

fn status_page(view: StatusView) -> impl Scene {
    let bars: Vec<_> = view.bars.into_iter().map(status_bar_cell).collect();
    let stats = (!view.attributes.is_empty()).then(|| {
        let attributes: Vec<_> = view.attributes.into_iter().map(attribute).collect();
        let combat: Vec<_> = view.combat.into_iter().map(combat_line).collect();
        EntityScene(bsn! {
            Node { flex_direction: FlexDirection::Column }
            @ignore_picking()
            Children [
                @section("ATTRIBUTES".into())
                --
                Node { column_gap: px(6) }
                @ignore_picking()
                Children [ {attributes} ]
                --
                @section("IN COMBAT".into())
                --
                Node { flex_wrap: FlexWrap::Wrap, column_gap: px(22) }
                @ignore_picking()
                Children [ {combat} ]
            ]
        })
    });
    bsn! {
        @page_column()
        Children [
            Node { flex_wrap: FlexWrap::Wrap, column_gap: px(22), row_gap: px(14) }
            @ignore_picking()
            Children [ {bars} ]
            --
            {stats}
            --
            Node {
                margin: {UiRect::top(px(18))},
                padding: {UiRect::axes(px(12), px(10))},
                border: px(1),
                border_radius: BorderRadius::all(px(6)),
            }
            BackgroundColor({INTIMACY.with_alpha(0.08)})
            BorderColor::all(INTIMACY.with_alpha(0.3))
            @ignore_picking()
            Children [ @chrome_text(view.tip, 12.0, ro_dark::BODY) ]
        ]
    }
}

fn skills_page(view: SkillsView) -> impl Scene {
    let tone = if view.points > 0 {
        ro_dark::RARE_RING
    } else {
        ro_dark::MUTED
    };
    let rows: Vec<_> = view.rows.into_iter().map(skill_row).collect();
    bsn! {
        @page_column()
        Children [
            Node { align_items: AlignItems::Center, margin: {UiRect::bottom(px(6))} }
            @ignore_picking()
            Children [
                @overline("SKILLS".into(), ro_dark::ACCENT)
                Node { flex_grow: 1.0 }
                --
                @chrome_text(format!("{} skill points", view.points), 12.0, tone)
            ]
            --
            {rows}
        ]
    }
}

/// The skill's icon with its cooldown over it. A skill that can be cast drags from
/// here onto the hotbar.
fn skill_icon(view: &SkillRow) -> impl Scene {
    let id = view.id;
    let learned = view.lock.is_none();
    let carried = learned && !view.passive;
    let stroke = if learned {
        ro_dark::SLOT_STROKE
    } else {
        ro_dark::MUTED_BRONZE
    };
    let art = view.icon.clone().map(|path| {
        EntityScene(bsn! {
            ImageNode { image: {path} }
            Node { width: px(24), height: px(24) }
            @ignore_picking()
        })
    });
    bsn! {
        SkillIcon(id)
        Node {
            width: px(40),
            height: px(40),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: px(1),
            border_radius: BorderRadius::all(px(6)),
            position_type: PositionType::Relative,
        }
        BackgroundColor(ro_dark::INSET)
        BorderColor::all(stroke)
        Pickable { should_block_lower: {carried}, is_hoverable: {carried} }
        on(drag_skill)
        Children [
            {art}
            --
            Cooldown(id)
            @outlined_text(String::new(), 13.0, ro_dark::STRONG)
            Node { position_type: PositionType::Absolute }
        ]
    }
}

fn skill_row(view: SkillRow) -> impl Scene {
    let id = view.id;
    let icon = skill_icon(&view);
    let passive = view.passive.then(|| {
        EntityScene(chip(
            "PASSIVE".into(),
            Color::WHITE.with_alpha(0.07),
            ro_dark::MUTED,
        ))
    });
    let auto = view.auto.then(|| {
        EntityScene(chip(
            "AUTO".into(),
            ro_dark::FROST_GLOW.with_alpha(0.14),
            ro_dark::SELECT_RING,
        ))
    });
    let lock = view
        .lock
        .map(|(text, tone)| EntityScene(chrome_text(text, 11.0, tone)));
    let cost = view
        .cost
        .map(|cost| EntityScene(chrome_text(cost, 11.0, ro_dark::SP)));
    let rank = view.rank.map(|rank| {
        EntityScene(bsn! {
            @row_button(ButtonArt::NORMAL, format!("Cast Lv {rank}"), 76.0)
            Act::Rank(id)
            on(on_button)
        })
    });
    let learn = view.can_learn.then(|| {
        EntityScene(bsn! {
            @row_button(ButtonArt::PRIMARY, "Learn", 56.0)
            Act::Learn(id)
            on(on_button)
        })
    });
    let cast = view.can_cast.then(|| {
        EntityScene(bsn! {
            @row_button(ButtonArt::NORMAL, "Use now", 68.0)
            Act::Cast(id)
            on(on_button)
        })
    });
    bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: px(12),
            padding: {UiRect::axes(px(4), px(10))},
            border: {UiRect::bottom(px(1))},
        }
        BorderColor::all(ro_dark::INSET_RULE)
        @ignore_picking()
        Children [
            @icon
            --
            Node {
                flex_grow: 1.0,
                flex_basis: px(0),
                min_width: px(0),
                flex_direction: FlexDirection::Column,
            }
            @ignore_picking()
            Children [
                Node { align_items: AlignItems::Center, column_gap: px(8) }
                @ignore_picking()
                Children [
                    @chrome_text(view.name, 14.0, ro_dark::STRONG)
                    --
                    @chrome_text(view.level, 11.0, ro_dark::TONE_OK)
                    --
                    {passive}
                    --
                    {auto}
                ]
                --
                @chrome_text(view.about, 12.0, ro_dark::MUTED)
                --
                {lock}
            ]
            --
            Node { flex_shrink: 0.0, align_items: AlignItems::Center, column_gap: px(8) }
            @ignore_picking()
            Children [ {cost} -- {rank} -- {learn} -- {cast} ]
        ]
    }
}

fn tactics_page(view: TacticsView) -> impl Scene {
    let body = view
        .body
        .map(|body| EntityScene(tactics_body(body, view.name.to_uppercase())));
    let (state, tone) = view.state;
    bsn! {
        @page_column()
        Children [
            {body}
            --
            Node { align_items: AlignItems::Center, column_gap: px(10), margin: {UiRect::top(px(20))} }
            @ignore_picking()
            Children [
                @chrome_text(state, 12.0, tone)
                Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0) }
                --
                @wide_button(ButtonArt::NORMAL, "Undo Changes", 112.0)
                @usable(view.dirty, Color::WHITE)
                Act::Undo
                on(on_button)
                --
                @fit_button(
                    ButtonArt::PRIMARY,
                    chrome_text(format!("Teach {}", view.name), 13.0, ro_dark::BUTTON_LABEL),
                )
                @usable(view.can_teach, Color::WHITE)
                Act::Teach
                on(on_button)
            ]
        ]
    }
}

fn stance_segment(
    label: &'static str,
    about: &'static str,
    stance: Stance,
    held: Stance,
) -> impl Scene {
    let active = stance == held;
    let (ink, _) = tab_text(active);
    let act = Act::Stance(stance);
    bsn! {
        @segment(active)
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            min_width: px(0),
            height: px(48),
            padding: {UiRect::all(px(0))},
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            row_gap: px(1),
        }
        act
        on(on_click)
        Children [
            @chrome_text(label.into(), 12.0, ink)
            --
            @chrome_text(about.into(), 10.0, ro_dark::FAINT)
        ]
    }
}

/// A slider `width` px wide with its value in words on the right.
fn dial(view: DialView, width: f32) -> impl Scene {
    let readout_of = SliderOf(view.dial);
    bsn! {
        Node {
            width: px(width),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            column_gap: px(10),
        }
        @ignore_picking()
        Children [
            @slider(view.dial, view.fraction, true)
            on(on_dial_press)
            on(on_dial_drag)
            --
            Node { width: px(54), flex_shrink: 0.0, justify_content: JustifyContent::FlexEnd }
            @ignore_picking()
            Children [
                SliderPart::Readout
                readout_of
                @chrome_text(view.readout, 12.0, ro_dark::STRONG)
            ]
        ]
    }
}

fn tactic_row(view: TacticRow) -> impl Scene {
    let control: Box<dyn Scene> = match view.control {
        Control::Dial(view) => Box::new(dial(view, 260.0)),
        Control::Flag(flag, enabled) => {
            let act = Act::Flip(flag);
            Box::new(bsn! {
                act
                Node {}
                Pickable
                on(on_click)
                Children [ @switch(enabled) ]
            })
        }
    };
    bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: px(12),
            padding: {UiRect::axes(px(4), px(10))},
            border: {UiRect::bottom(px(1))},
        }
        BorderColor::all(ro_dark::INSET_RULE)
        @ignore_picking()
        Children [
            Node {
                flex_grow: 1.0,
                flex_basis: px(0),
                min_width: px(0),
                flex_direction: FlexDirection::Column,
            }
            @ignore_picking()
            Children [
                @chrome_text(view.label.into(), 13.0, ro_dark::STRONG)
                --
                @chrome_text(view.about, 11.0, ro_dark::MUTED)
            ]
            --
            @control
        ]
    }
}

fn mode_segment(label: &'static str, skill_id: u32, mode: Mode, active: bool) -> impl Scene {
    let act = Act::Mode(skill_id, mode);
    bsn! {
        @even_segment(label, active)
        Node {
            flex_grow: 0.0,
            flex_basis: Val::Auto,
            height: px(24),
            padding: {UiRect::horizontal(px(8))},
        }
        act
        on(on_click)
    }
}

fn condition(view: DialView) -> impl Scene {
    let label = view.label.clone();
    bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: px(10),
            padding: {UiRect::left(px(28))},
        }
        @ignore_picking()
        Children [
            @chrome_text(label, 12.0, ro_dark::LABEL)
            Node { width: px(190), flex_shrink: 0.0 }
            --
            @dial(view, 300.0)
        ]
    }
}

fn rule_card(view: RuleView) -> impl Scene {
    let id = view.skill_id;
    // The chevron art points right; this one points up.
    let turn = UiTransform::from_rotation(Rot2::degrees(-90.0));
    let raise = (view.place > 1).then(|| {
        EntityScene(bsn! {
            @ghost_button("chev")
            turn
            Act::Raise(id)
            on(on_button)
        })
    });
    let conditions: Vec<_> = view.dials.into_iter().map(condition).collect();
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(8),
            margin: {UiRect::top(px(8))},
            padding: {UiRect::axes(px(12), px(10))},
            border: px(1),
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor(ro_dark::SLOT)
        BorderColor::all(ro_dark::SLOT_STROKE)
        @ignore_picking()
        Children [
            Node { align_items: AlignItems::Center, column_gap: px(8) }
            @ignore_picking()
            Children [
                @chrome_text(view.place.to_string(), 12.0, ro_dark::ACCENT)
                Node { width: px(20), flex_shrink: 0.0 }
                --
                @chrome_text(view.name, 13.0, ro_dark::STRONG)
                Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0) }
                --
                @mode_segment("Manual", id, Mode::Manual, !view.auto)
                --
                @mode_segment("Auto", id, Mode::Auto, view.auto)
                --
                {raise}
            ]
            --
            {conditions}
        ]
    }
}

/// A monster list: its label, then a chip per class id that removes it on a click.
fn mob_list(label: &'static str, list: MobList, ids: Vec<u32>) -> impl Scene {
    let chips: Vec<_> = ids
        .into_iter()
        .map(|id| {
            bsn! {
                @row_button(ButtonArt::NORMAL, format!("#{id} \u{d7}"), 68.0)
                @usable(true, DANGER_TINT)
                Act::DropMob(list, id)
                on(on_button)
            }
        })
        .collect();
    bsn! {
        Node {
            flex_wrap: FlexWrap::Wrap,
            align_items: AlignItems::Center,
            column_gap: px(6),
            row_gap: px(6),
            min_height: px(24),
        }
        @ignore_picking()
        Children [
            @chrome_text(label.into(), 12.0, ro_dark::MUTED)
            Node { width: px(84), flex_shrink: 0.0 }
            --
            {chips}
        ]
    }
}

fn tactics_body(view: TacticsBody, name: String) -> impl Scene {
    let rows: Vec<_> = view.rows.into_iter().map(tactic_row).collect();
    let rules: Vec<_> = view.rules.into_iter().map(rule_card).collect();
    bsn! {
        Node { flex_direction: FlexDirection::Column }
        @ignore_picking()
        Children [
            @overline(format!("HOW {name} FIGHTS"), ro_dark::ACCENT)
            --
            Node { column_gap: px(6), margin: {UiRect::top(px(8))} }
            @ignore_picking()
            Children [
                @stance_segment("Passive", "Never starts a fight", Stance::Passive, view.stance)
                --
                @stance_segment("Assist", "Helps with your fights", Stance::Assist, view.stance)
                --
                @stance_segment(
                    "Aggressive",
                    "Attacks anything nearby",
                    Stance::Aggressive,
                    view.stance,
                )
            ]
            --
            {rows}
            --
            @section("SKILL RULES \u{b7} TOP RUNS FIRST".into())
            Node { margin: {UiRect::new(px(0), px(0), px(18), px(0))} }
            --
            {rules}
            --
            @section("MONSTERS".into())
            Node { margin: {UiRect::new(px(0), px(0), px(18), px(8))} }
            --
            Node { flex_direction: FlexDirection::Column, row_gap: px(6) }
            @ignore_picking()
            Children [
                @mob_list("Only attack", MobList::Only, view.only)
                --
                @mob_list("Never attack", MobList::Never, view.never)
                --
                Node { align_items: AlignItems::Center, column_gap: px(6), margin: {UiRect::top(px(4))} }
                @ignore_picking()
                Children [
                    @chrome_text("Monster ID".into(), 12.0, ro_dark::MUTED)
                    Node { width: px(84), flex_shrink: 0.0 }
                    --
                    MobField
                    @number_field(String::new(), 6)
                    Node { width: px(72) }
                    --
                    @row_button(ButtonArt::NORMAL, "Only attack", 84.0)
                    Act::AddMob(MobList::Only)
                    on(on_button)
                    --
                    @row_button(ButtonArt::NORMAL, "Never attack", 88.0)
                    Act::AddMob(MobList::Never)
                    on(on_button)
                ]
            ]
        ]
    }
}

/// Where the prompt is mounted: a layer of its own, so it covers the whole screen.
pub(super) fn confirm_host() -> impl Scene {
    bsn! {
        Host::Confirm
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
        }
        @ignore_picking()
    }
}

pub(super) fn confirm(view: ConfirmView) -> impl Scene {
    let sure = view.sure.map(|enabled| {
        EntityScene(bsn! {
            Act::Sure
            Node { align_items: AlignItems::Center, column_gap: px(10), margin: {UiRect::top(px(14))} }
            Pickable
            on(on_click)
            Children [
                @switch(enabled)
                --
                @chrome_text("I understand this can\u{2019}t be undone".into(), 12.0, ro_dark::LABEL)
            ]
        })
    });
    let armed = view.sure != Some(false);
    bsn! {
        @modal_layer(CONFIRM_Z)
        BackgroundColor({ro_dark::WINDOW_OUTLINE.with_alpha(0.6)})
        Children [
            @window_titled()
            Node {
                width: px(400.0 + 2.0 * WINDOW_SHADOW),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
            }
            Pickable
            Children [
                @window_title_bar(view.title, bsn! { Node {} @ignore_picking() })
                --
                @chrome_text(view.text, 13.0, ro_dark::BODY)
                Node { margin: {UiRect::top(px(8))} }
                --
                {sure}
                --
                Node {
                    justify_content: JustifyContent::FlexEnd,
                    column_gap: px(8),
                    margin: {UiRect::new(px(0), px(0), px(18), px(6))},
                }
                @ignore_picking()
                Children [
                    @fit_button(
                        ButtonArt::NORMAL,
                        chrome_text(view.keep, 13.0, ro_dark::BUTTON_LABEL),
                    )
                    Act::Cancel
                    on(on_button)
                    --
                    @fit_button(
                        ButtonArt::NORMAL,
                        chrome_text(view.go.into(), 13.0, ro_dark::BUTTON_LABEL),
                    )
                    @usable(armed, DANGER_TINT)
                    Act::Confirm
                    on(on_button)
                ]
            ]
        ]
    }
}

/// The menu a right click on the owner's companion raises.
pub(super) fn context_menu(pos: Vec2) -> impl Scene {
    bsn! {
        OwnerMenu
        Pickable
        GlobalZIndex(120)
        Node {
            position_type: PositionType::Absolute,
            left: px(pos.x),
            top: px(pos.y),
            flex_direction: FlexDirection::Column,
            padding: px(4),
            border: px(1),
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor(ro_dark::TOOLTIP_BODY)
        BorderColor::all(ro_dark::BUTTON_STROKE)
        Children [
            @menu_row(RowTone::Ghost, 30.0, "heart", "Homunculus", None)
            Act::Open
            on(on_button)
        ]
    }
}

/// A tinted glyph beside a word, for the plate's intimacy grade.
pub(super) fn glyph_word(icon: &'static str, word: String, tone: Color) -> impl Scene {
    bsn! {
        Node { align_items: AlignItems::Center, column_gap: px(3) }
        @ignore_picking()
        Children [ @glyph_icon(icon, 10.0, tone) -- @chrome_text(word, 10.0, tone) ]
    }
}
