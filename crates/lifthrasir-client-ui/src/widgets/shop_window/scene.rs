//! The Shop window scenes: the frame [`window`] spawns when a shop opens, and the
//! tabs, list, cart and footer that `rebuild_body` projects from their view-models.
//! Sizes follow the `NPC Shop` artboard of the Lifthrasir Widgets design.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{EditableText, EditableTextFilter, FontSize, FontSourceTemplate};
use bevy::ui_widgets::{Button, ControlOrientation, ScrollArea, TextInput};
use bevy_feathers::controls::FeathersScrollbar;
use lifthrasir_ui::theme::{self, ro_dark};
use lifthrasir_ui::widgets::chrome::{chrome_text, drag_window, glyph_icon, ignore_picking};
use lifthrasir_ui::widgets::primitives::{
    BUTTON_SHADOW, ButtonArt, HoverColors, INPUT_SHADOW, WINDOW_SHADOW, button_with, close_button,
    ghost_button, inset, segment, tab_text, window_title_bar, window_titled,
};

use crate::screens::shared::{
    DANGER_TINT, Ring, chip, link, overline, row_button, search_field, slot_frame, small_button,
    thousands, usable, weight_meter,
};
use crate::widgets::inventory_window::CAPACITY;
use crate::widgets::inventory_window::scene::slot_marks;
use crate::widgets::shop_window::view::{
    CartLineView, CartView, ChipTone, FooterView, ListView, QuickView, RowControl, RowView, TabView,
};
use crate::widgets::shop_window::{
    ShopButtonAction, ShopMount, ShopQtyField, ShopRow, ShopScroll, ShopSearchField,
    ShopWindowRoot, ShopWindowTitlebar, on_shop_button, on_shop_close_button,
    on_shop_row_secondary_click,
};

const WINDOW_LEFT: f32 = 150.0;
const WINDOW_TOP: f32 = 40.0;
const WINDOW_WIDTH: f32 = 948.0;
const LIST_COLUMN: f32 = 556.0;
const LIST_HEIGHT: f32 = 384.0;
const CART_WIDTH: f32 = 324.0;
const CART_HEIGHT: f32 = 480.0;
const SLOT: f32 = 38.0;
const ICON: f32 = 28.0;
const CART_SLOT: f32 = 30.0;
const CART_ICON: f32 = 22.0;
const PRICE_COLUMN: f32 = 74.0;
const CONTROL_COLUMN: f32 = 168.0;
const STEP_BUTTON: f32 = 24.0;
/// A stack never holds more than 30,000, so five digits cover every quantity.
const QTY_DIGITS: usize = 5;

/// The whole window: the frame, the title bar, the search field and the four empty
/// [`ShopMount`]s.
pub fn window(title: String) -> impl Scene {
    bsn! {
        ShopWindowRoot
        @window_titled()
        Node {
            position_type: PositionType::Absolute,
            left: px(WINDOW_LEFT),
            top: px(WINDOW_TOP),
            width: px(WINDOW_WIDTH + 2.0 * WINDOW_SHADOW),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
        }
        Pickable
        Children [
            @title_bar(title)
            --
            Node {
                column_gap: px(20),
                align_items: AlignItems::FlexStart,
                margin: {UiRect::top(px(10))},
            }
            @ignore_picking()
            Children [
                Node {
                    width: px(LIST_COLUMN),
                    flex_shrink: 0.0,
                    flex_direction: FlexDirection::Column,
                }
                @ignore_picking()
                Children [
                    Node { align_items: AlignItems::Center, column_gap: px(8) }
                    @ignore_picking()
                    Children [
                        ShopMount::Tabs
                        Node { flex_shrink: 0.0 }
                        @ignore_picking()
                        --
                        Node { flex_grow: 1.0 }
                        @ignore_picking()
                        --
                        @search_field::<ShopSearchField>("Search items")
                        Node {
                            width: px(200.0 + 2.0 * INPUT_SHADOW),
                            flex_grow: 0.0,
                            flex_shrink: 0.0,
                        }
                    ]
                    --
                    ShopMount::List
                    Node { flex_direction: FlexDirection::Column, margin: {UiRect::top(px(12))} }
                    @ignore_picking()
                ]
                --
                ShopMount::Cart
                @inset()
                Node {
                    width: px(CART_WIDTH),
                    height: px(CART_HEIGHT),
                    flex_shrink: 0.0,
                    flex_direction: FlexDirection::Column,
                }
                @ignore_picking()
            ]
            --
            ShopMount::Footer
            Node {
                flex_direction: FlexDirection::Column,
                margin: {UiRect::new(px(0), px(0), px(14), px(8))},
            }
            @ignore_picking()
        ]
    }
}

fn title_bar(title: String) -> impl Scene {
    bsn! {
        ShopWindowTitlebar
        @window_title_bar(title, bsn! { @close_button() on(on_shop_close_button) })
        Pickable
        on(drag_window::<ShopWindowTitlebar, ShopWindowRoot>)
    }
}

pub(super) fn tabs(views: [TabView; 2]) -> impl Scene {
    let buttons: Vec<_> = views.into_iter().map(tab_button).collect();
    bsn! {
        Node { column_gap: px(4) }
        @ignore_picking()
        Children [ {buttons} ]
    }
}

/// A tab with the number of lines in its cart.
fn tab_button(view: TabView) -> impl Scene {
    let active = view.active;
    let (label, _) = tab_text(active);
    let action = ShopButtonAction::SwitchTab(view.tab);
    let lines = (view.lines > 0)
        .then(|| EntityScene(chrome_text(view.lines.to_string(), 11.0, ro_dark::ACCENT)));
    bsn! {
        Button
        action
        @segment(active)
        Node { width: px(110), height: px(28), justify_content: JustifyContent::Center }
        on(on_shop_button)
        Children [ @chrome_text(view.tab.label().into(), 13.0, label) -- {lines} ]
    }
}

/// The quick-action strip over the stock panel.
pub(super) fn list(view: ListView) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(10) }
        @ignore_picking()
        Children [
            @quick_strip(view.quick)
            --
            @stock_panel(view.buy, view.rows, view.message, view.scroll)
        ]
    }
}

fn quick_strip(view: QuickView) -> impl Scene {
    let (label, enabled) = (view.label, view.enabled);
    bsn! {
        @inset()
        Node {
            height: px(46),
            padding: {UiRect::horizontal(px(12))},
            align_items: AlignItems::Center,
            column_gap: px(10),
        }
        @ignore_picking()
        Children [
            @overline(view.title.into(), ro_dark::ACCENT)
            Node { flex_shrink: 0.0 }
            --
            Node {
                flex_grow: 1.0,
                flex_basis: px(0),
                min_width: px(0),
                overflow: {Overflow::clip()},
            }
            @ignore_picking()
            Children [
                @chrome_text(view.text, 12.0, ro_dark::LABEL)
                TextLayout { linebreak: LineBreak::NoWrap }
            ]
            --
            @row_button(ButtonArt::NORMAL, label, 96.0)
            @usable(enabled, Color::WHITE)
            ShopButtonAction::AddLoot
            on(on_shop_button)
        ]
    }
}

/// The stock list: a column header over a fixed-height scroll viewport of rows, with
/// the scrollbar pinned to its right.
fn stock_panel(buy: bool, rows: Vec<RowView>, message: Option<String>, scroll: Vec2) -> impl Scene {
    let price_head = if buy { "PRICE" } else { "PAYS" };
    let rows: Vec<_> = rows.into_iter().map(stock_row).collect();
    let message = message.map(|text| EntityScene(list_message(text)));
    bsn! {
        @inset()
        Node { height: px(LIST_HEIGHT), flex_direction: FlexDirection::Column }
        @ignore_picking()
        Children [
            @list_head(price_head)
            --
            Node { flex_grow: 1.0, min_height: px(0), position_type: PositionType::Relative }
            @ignore_picking()
            Children [
                #list
                ShopScroll::List
                ScrollArea
                ScrollPosition(scroll)
                Pickable
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0), top: px(0), right: px(0), bottom: px(0),
                    flex_direction: FlexDirection::Column,
                    padding: {UiRect::new(px(4), px(14), px(0), px(0))},
                    overflow: {Overflow::scroll_y()},
                }
                Children [ {rows} ]
                --
                @FeathersScrollbar { @target: #list, @orientation: {ControlOrientation::Vertical} }
                Node {
                    position_type: PositionType::Absolute,
                    right: px(4),
                    top: px(6),
                    bottom: px(6),
                    width: px(6),
                }
                --
                {message}
            ]
        ]
    }
}

fn list_head(price: &'static str) -> impl Scene {
    bsn! {
        Node {
            height: px(26),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            column_gap: px(10),
            padding: {UiRect::new(px(14), px(24), px(0), px(0))},
            border: {UiRect::bottom(px(1))},
        }
        BorderColor::all(ro_dark::INSET_RULE)
        @ignore_picking()
        Children [
            Node { width: px(SLOT), flex_shrink: 0.0 }
            @ignore_picking()
            --
            @overline("ITEM".into(), ro_dark::ACCENT)
            Node { flex_grow: 1.0 }
            --
            @column_head(price, PRICE_COLUMN)
            --
            @column_head("QUANTITY", CONTROL_COLUMN)
        ]
    }
}

/// A right-aligned column caption `width` px wide.
fn column_head(text: &'static str, width: f32) -> impl Scene {
    bsn! {
        Node { width: px(width), flex_shrink: 0.0, justify_content: JustifyContent::FlexEnd }
        @ignore_picking()
        Children [ @overline(text.into(), ro_dark::ACCENT) ]
    }
}

fn list_message(text: String) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(120),
            justify_content: JustifyContent::Center,
        }
        @ignore_picking()
        Children [ @chrome_text(text, 12.0, ro_dark::MUTED) ]
    }
}

/// One stock row: slot, name with its tag over a detail line, unit price, then the
/// quantity controls. Right-click inspects the item.
fn stock_row(view: RowView) -> impl Scene {
    let (key, ring) = (view.key, view.cell.ring);
    let name_color = match (view.control, ring) {
        (RowControl::Locked(_), _) => ro_dark::MUTED,
        (RowControl::Add(_), Ring::Rare) => ro_dark::RARE_RING,
        (RowControl::Add(_), _) => ro_dark::STRONG,
    };
    let colors = HoverColors {
        rest: (Color::NONE, ro_dark::INSET_RULE),
        hover: (Color::WHITE.with_alpha(0.02), ro_dark::INSET_RULE),
    };
    let marks = slot_marks(&view.cell, ICON, false);
    let tag = view.chip.map(|(text, tone)| EntityScene(tag(text, tone)));
    let price = format!("{} z", thousands(view.price.into()));
    let control = row_control(key, view.control);
    bsn! {
        ShopRow(key)
        Hovered
        colors
        Node {
            height: px(48),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            column_gap: px(10),
            padding: {UiRect::horizontal(px(10))},
            border: {UiRect::bottom(px(1))},
        }
        BackgroundColor(Color::NONE)
        BorderColor::all(ro_dark::INSET_RULE)
        Pickable
        on(on_shop_row_secondary_click)
        Children [
            @slot_frame(SLOT, ring)
            @ignore_picking()
            Children [ {marks} ]
            --
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                flex_basis: px(0),
                min_width: px(0),
                overflow: {Overflow::clip()},
            }
            @ignore_picking()
            Children [
                Node { align_items: AlignItems::Center, column_gap: px(6) }
                @ignore_picking()
                Children [
                    @chrome_text(view.name, 13.0, name_color)
                    TextLayout { linebreak: LineBreak::NoWrap }
                    --
                    {tag}
                ]
                --
                @chrome_text(view.sub, 11.0, ro_dark::MUTED)
                TextLayout { linebreak: LineBreak::NoWrap }
            ]
            --
            Node {
                width: px(PRICE_COLUMN),
                flex_shrink: 0.0,
                justify_content: JustifyContent::FlexEnd,
            }
            @ignore_picking()
            Children [ @chrome_text(price, 13.0, ro_dark::BRONZE_LIGHT) ]
            --
            Node {
                width: px(CONTROL_COLUMN),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexEnd,
                column_gap: px(4),
            }
            @ignore_picking()
            Children [ {control} ]
        ]
    }
}

fn tag(text: String, tone: ChipTone) -> impl Scene {
    let (fill, ink) = match tone {
        ChipTone::Cart => (ro_dark::PRIMARY, ro_dark::CHECK_MARK),
        ChipTone::Cards => (ro_dark::STATUS_FULL.with_alpha(0.18), ro_dark::DANGER_GLOW),
        ChipTone::Rare => (ro_dark::RARE_RING.with_alpha(0.16), ro_dark::RARE_RING),
    };
    chip(text, fill, ink)
}

/// The stepper, quantity field and Add button of a row, or the reason it has none.
fn row_control(key: u32, control: RowControl) -> Vec<Box<dyn Scene>> {
    match control {
        RowControl::Add(qty) => vec![
            Box::new(step_button("minus", ShopButtonAction::RowDec(key))),
            Box::new(qty_field(key, qty)),
            Box::new(step_button("plus", ShopButtonAction::RowInc(key))),
            Box::new(bsn! {
                @row_button(ButtonArt::NORMAL, "Add", 44.0)
                ShopButtonAction::AddRow(key)
                on(on_shop_button)
            }),
        ],
        RowControl::Locked(why) => vec![
            Box::new(glyph_icon("lock", 11.0, ro_dark::MUTED)),
            Box::new(chrome_text(why.into(), 11.0, ro_dark::MUTED)),
        ],
    }
}

fn step_button(icon: &'static str, action: ShopButtonAction) -> impl Scene {
    bsn! {
        @small_button(icon, STEP_BUTTON)
        action
        on(on_shop_button)
    }
}

/// The typed quantity of a row. It lives in the field alone: the row's stepper edits
/// it and Add reads it.
fn qty_field(key: u32, qty: u32) -> impl Scene {
    let field = EditableText {
        max_characters: Some(QTY_DIGITS),
        ..EditableText::new(qty.to_string())
    };
    bsn! {
        ShopQtyField(key)
        field
        TextInput
        EditableTextFilter::new(|c| c.is_ascii_digit())
        TextFont {
            font: FontSourceTemplate::Handle(theme::FONT_BODY),
            font_size: {FontSize::Px(13.0)},
        }
        TextColor(ro_dark::BODY)
        TextLayout { justify: Justify::Center }
        Node {
            width: px(46),
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

/// The cart column: its header, the scrolling lines, then the totals and the submit
/// button.
pub(super) fn cart(view: CartView) -> impl Scene {
    let (title, hint) = if view.buy {
        ("CART", "Add goods from the list to fill your cart.")
    } else {
        (
            "TO SELL",
            "Add items from your bag. Favorites can\u{2019}t be sold here.",
        )
    };
    let count = match view.lines.len() {
        0 => String::new(),
        1 => "1 line".to_string(),
        lines => format!("{lines} lines"),
    };
    let empty = view.lines.is_empty();
    let scroll = view.scroll;
    let lines: Vec<_> = view.lines.into_iter().map(cart_line).collect();
    let hint = empty.then(|| EntityScene(cart_hint(hint)));
    bsn! {
        Node { width: percent(100), height: percent(100), flex_direction: FlexDirection::Column }
        @ignore_picking()
        Children [
            @cart_head(title, count, !empty)
            --
            Node { flex_grow: 1.0, min_height: px(0), position_type: PositionType::Relative }
            @ignore_picking()
            Children [
                #lines
                ShopScroll::Cart
                ScrollArea
                ScrollPosition(scroll)
                Pickable
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0), top: px(0), right: px(0), bottom: px(0),
                    flex_direction: FlexDirection::Column,
                    padding: {UiRect::new(px(4), px(14), px(0), px(0))},
                    overflow: {Overflow::scroll_y()},
                }
                Children [ {lines} -- {hint} ]
                --
                @FeathersScrollbar { @target: #lines, @orientation: {ControlOrientation::Vertical} }
                Node {
                    position_type: PositionType::Absolute,
                    right: px(4),
                    top: px(6),
                    bottom: px(6),
                    width: px(6),
                }
            ]
            --
            @cart_foot(view.warnings, view.summary, view.error, view.cta, view.cta_enabled)
        ]
    }
}

fn cart_head(title: &'static str, count: String, clearable: bool) -> impl Scene {
    let ink = if clearable {
        ro_dark::FROST
    } else {
        ro_dark::FAINT
    };
    bsn! {
        Node {
            height: px(40),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            column_gap: px(8),
            padding: {UiRect::horizontal(px(12))},
            border: {UiRect::bottom(px(1))},
        }
        BorderColor::all(ro_dark::INSET_RULE)
        @ignore_picking()
        Children [
            @overline(title.into(), ro_dark::ACCENT)
            --
            @chrome_text(count, 12.0, ro_dark::STRONG)
            Node { flex_grow: 1.0 }
            --
            Button
            @link("Clear")
            TextColor(ink)
            Pickable { should_block_lower: {clearable}, is_hoverable: {clearable} }
            ShopButtonAction::ClearCart
            on(on_shop_button)
        ]
    }
}

fn cart_hint(text: &'static str) -> impl Scene {
    bsn! {
        Node { margin: {UiRect::new(px(16), px(16), px(80), px(0))}, justify_content: JustifyContent::Center }
        @ignore_picking()
        Children [
            @chrome_text(text.into(), 12.0, ro_dark::MUTED)
            TextLayout { justify: Justify::Center }
        ]
    }
}

fn cart_line(view: CartLineView) -> impl Scene {
    let (key, ring, at_max) = (view.key, view.cell.ring, view.at_max);
    let name_color = if ring == Ring::Rare {
        ro_dark::RARE_RING
    } else {
        ro_dark::STRONG
    };
    let marks = slot_marks(&view.cell, CART_ICON, false);
    let qty = thousands(view.qty.into());
    bsn! {
        Node {
            height: px(44),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            column_gap: px(8),
            padding: {UiRect::horizontal(px(8))},
            border: {UiRect::bottom(px(1))},
        }
        BorderColor::all(ro_dark::INSET_RULE)
        @ignore_picking()
        Children [
            @slot_frame(CART_SLOT, ring)
            @ignore_picking()
            Children [ {marks} ]
            --
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                flex_basis: px(0),
                min_width: px(0),
                overflow: {Overflow::clip()},
            }
            @ignore_picking()
            Children [
                @chrome_text(view.name, 12.0, name_color)
                TextLayout { linebreak: LineBreak::NoWrap }
                --
                @chrome_text(view.sub, 11.0, ro_dark::MUTED)
            ]
            --
            @step_button("minus", ShopButtonAction::DecQty(key))
            --
            Node { min_width: px(30), flex_shrink: 0.0, justify_content: JustifyContent::Center }
            @ignore_picking()
            Children [ @chrome_text(qty, 12.0, ro_dark::STRONG) ]
            --
            @step_button("plus", ShopButtonAction::IncQty(key))
            @usable(!at_max, Color::WHITE)
            --
            @ghost_button("close")
            ShopButtonAction::RemoveLine(key)
            on(on_shop_button)
        ]
    }
}

/// The sell warning when it is up, the totals, the shortfall and the submit button.
fn cart_foot(
    warnings: Vec<String>,
    summary: Vec<(&'static str, String, Color)>,
    error: Option<&'static str>,
    cta: String,
    cta_enabled: bool,
) -> impl Scene {
    let warning = (!warnings.is_empty()).then(|| EntityScene(sell_warning(warnings)));
    let totals: Vec<_> = summary.into_iter().map(summary_row).collect();
    let error = error.map(|text| EntityScene(chrome_text(text.into(), 11.0, ro_dark::STATUS_FULL)));
    bsn! {
        Node {
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            padding: {UiRect::new(px(12), px(12), px(10), px(12))},
            border: {UiRect::top(px(1))},
        }
        BorderColor::all(ro_dark::INSET_RULE)
        @ignore_picking()
        Children [
            {warning}
            --
            {totals}
            --
            {error}
            --
            @button_with(ButtonArt::PRIMARY, chrome_text(cta, 14.0, ro_dark::BUTTON_LABEL))
            Node {
                height: px(34.0 + 2.0 * BUTTON_SHADOW),
                flex_grow: 0.0,
                margin: {UiRect::new(
                    px(-BUTTON_SHADOW),
                    px(-BUTTON_SHADOW),
                    px(10.0 - BUTTON_SHADOW),
                    px(-BUTTON_SHADOW),
                )},
            }
            @usable(cta_enabled, Color::WHITE)
            ShopButtonAction::Submit
            on(on_shop_button)
        ]
    }
}

fn summary_row((label, value, ink): (&'static str, String, Color)) -> impl Scene {
    bsn! {
        Node {
            height: px(19),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
        }
        @ignore_picking()
        Children [
            @chrome_text(label.into(), 12.0, ro_dark::LABEL)
            --
            @chrome_text(value, 12.0, ink)
        ]
    }
}

/// The "Check before you sell" box: one line per risky item, then Back and Sell Anyway.
fn sell_warning(lines: Vec<String>) -> impl Scene {
    let lines: Vec<_> = lines
        .into_iter()
        .map(|line| chrome_text(line, 12.0, ro_dark::BODY))
        .collect();
    let fill = ro_dark::HP.with_alpha(0.12);
    let stroke = ro_dark::STATUS_FULL.with_alpha(0.55);
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(2),
            padding: {UiRect::axes(px(12), px(10))},
            margin: {UiRect::bottom(px(10))},
            border: px(1),
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor(fill)
        BorderColor::all(stroke)
        @ignore_picking()
        Children [
            @chrome_text("Check before you sell".into(), 12.0, ro_dark::DANGER_GLOW)
            --
            {lines}
            --
            Node {
                margin: {UiRect::top(px(8))},
                justify_content: JustifyContent::FlexEnd,
                column_gap: px(8),
            }
            @ignore_picking()
            Children [
                @row_button(ButtonArt::NORMAL, "Back", 52.0)
                ShopButtonAction::CancelConfirm
                on(on_shop_button)
                --
                @row_button(ButtonArt::NORMAL, "Sell Anyway", 92.0)
                @usable(true, DANGER_TINT)
                ShopButtonAction::ConfirmTrade
                on(on_shop_button)
            ]
        ]
    }
}

/// The zeny, weight and bag read-outs under both columns.
pub(super) fn footer(view: FooterView) -> impl Scene {
    let zeny = format!("{} z", thousands(view.zeny.into()));
    let bag = format!("Bag {} / {CAPACITY}", view.bag);
    bsn! {
        Node { align_items: AlignItems::Center, column_gap: px(24) }
        @ignore_picking()
        Children [
            Node { align_items: AlignItems::Baseline, column_gap: px(8), flex_shrink: 0.0 }
            @ignore_picking()
            Children [
                @chrome_text("Zeny".into(), 12.0, ro_dark::ACCENT)
                --
                @chrome_text(zeny, 16.0, ro_dark::BRONZE_LIGHT)
            ]
            --
            Node { width: px(300), flex_shrink: 0.0 }
            @ignore_picking()
            Children [ @weight_meter(view.weight, view.max_weight) ]
            --
            @chrome_text(bag, 12.0, ro_dark::MUTED)
            Node { flex_grow: 1.0 }
            --
            @chrome_text("Right-click an item for details".into(), 11.0, ro_dark::MUTED)
        ]
    }
}
