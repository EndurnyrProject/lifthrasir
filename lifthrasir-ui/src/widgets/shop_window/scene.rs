//! Idiomatic BSN chrome for the NPC shop window: a draggable, single-instance
//! window (design `2026-07-07-npc-shops` §5.4). [`window`] builds the whole chrome
//! — root, titlebar, and an (initially empty) body — as one `bsn!` tree; Task 7
//! fills the body region.

use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui_widgets::{ControlOrientation, ScrollArea};
use bevy_feathers::controls::{FeathersButton, FeathersScrollbar};
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor, ThemeTextColor};
use game_engine::domain::assets::item_icon_path;
use game_engine::domain::inventory::Inventory;
use game_engine::infrastructure::item::ItemDb;

use crate::theme;
use crate::theme::feathers_theme::{
    TOKEN_ACCENT, TOKEN_TEXT, TOKEN_TEXT_DIM, TOKEN_TITLEBAR_BG, TOKEN_WINDOW_BG,
    TOKEN_WINDOW_BORDER,
};
use crate::widgets::chrome::{body_container, drag_window, glyph_icon, ignore_picking};

use super::{
    on_shop_button, on_shop_close_button, Selection, ShopButtonAction, ShopSession, ShopTab,
    ShopWindowBody, ShopWindowRoot, ShopWindowTitlebar,
};

const WINDOW_LEFT: f32 = 340.0;
const WINDOW_TOP: f32 = 90.0;
const WINDOW_WIDTH: f32 = 732.0;

/// The whole window: the root card (draggable, carries [`ShopWindowRoot`]), its
/// titlebar, and an empty body placeholder for Task 7's grid/detail/cart/footer.
pub fn window(title: String) -> impl Scene {
    bsn! {
        ShopWindowRoot
        Node {
            position_type: PositionType::Absolute,
            left: px(WINDOW_LEFT),
            top: px(WINDOW_TOP),
            width: px(WINDOW_WIDTH),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
            border: px(1),
            border_radius: BorderRadius::all(px(9)),
        }
        ThemeBackgroundColor({TOKEN_WINDOW_BG})
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        Pickable
        Children [ titlebar(title), body_container::<ShopWindowBody>(UiRect::axes(Val::Px(14.0), Val::Px(12.0))) ]
    }
}

fn titlebar(title: String) -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(6),
            padding: {UiRect::axes(px(10), px(7))},
            border: {UiRect { bottom: Val::Px(1.0), ..default() }},
        }
        ThemeBackgroundColor({TOKEN_TITLEBAR_BG})
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        ShopWindowTitlebar
        Pickable
        on(drag_window::<ShopWindowTitlebar, ShopWindowRoot>)
        Children [
            glyph_icon("rune", 13.0, theme::GOLD),
            (
                Text(title)
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/cinzel.ttf"),
                    font_size: {FontSize::Px(13.0)},
                }
                ThemeTextColor({TOKEN_TEXT})
                Node { flex_grow: 1.0 }
                ignore_picking()
            ),
            (
                @FeathersButton { @caption: bsn! { glyph_icon("close", 11.0, theme::TEXT_DIM) } }
                Node { width: px(20), height: px(16) }
                on(on_shop_close_button)
            ),
        ]
    }
}

// The (initially empty) body region is `chrome::body_container`; `rebuild_body` fills
// it with a single [`body`] child on every `ShopSession` change, so the container's own
// `row_gap` never applied (it needs two siblings) and is dropped.
// ---------------------------------------------------------------------------
// Body: tab strip, grid, detail panel, cart, footer (design §5.4).
//
// `bsn!` scenes own their data, so every view-model below is prepared as plain
// owned values (`String`/`u32`/...) *before* being handed to a `bsn!` block —
// no `ShopSession`/`ItemDb`/`Inventory` reference ever crosses into one.
// ---------------------------------------------------------------------------

/// Fixed width of the right side column (detail + basket), mirrors the
/// mockup's `.sh-side-inner`.
const SIDE_COLUMN_WIDTH: f32 = 286.0;
/// Fixed height shared by the stock pane and the side column, so the window
/// never grows with content — both panes scroll internally instead (mirrors
/// the mockup's fixed `.sh-grid-wrap`/`.sh-side-inner` heights).
const PANE_HEIGHT: f32 = 388.0;

/// One item row in the stock list: a sprite well, the item name (with an
/// optional refine/stock subtitle), an in-cart badge, and the unit price.
struct RowView {
    icon: Option<String>,
    name: String,
    price: u32,
    refine: Option<u8>,
    owned: Option<u32>,
    cart_qty: Option<u32>,
    selected: bool,
    action: ShopButtonAction,
}

struct DetailView {
    icon: Option<String>,
    name: String,
    price: u32,
    buy: bool,
    amount: Option<u32>,
    cards: Vec<bool>,
    description: Option<String>,
    pending_qty: u32,
}

#[derive(Clone)]
struct CartLineView {
    key: u32,
    name: String,
    unit_price: u32,
    qty: u32,
}

/// The basket block's view-model: the active tab's cart lines plus its
/// subtotal, bundled so [`cart_panel`] doesn't need the whole `ShopSession`.
struct CartView {
    buy: bool,
    lines: Vec<CartLineView>,
    total: u64,
}

struct FooterView {
    zeny: u32,
    total: u64,
    buy: bool,
    warning: bool,
    cta_label: String,
    cta_enabled: bool,
}

fn icon_path(item_db: Option<&ItemDb>, nameid: u32, identified: bool) -> Option<String> {
    item_db
        .and_then(|db| db.icon_resource(nameid, identified))
        .map(item_icon_path)
}

fn item_name(item_db: Option<&ItemDb>, nameid: u32, identified: bool) -> String {
    item_db
        .and_then(|db| db.name(nameid, identified))
        .map(str::to_string)
        .unwrap_or_else(|| format!("#{nameid}"))
}

/// Whether the Sell slot at `inventory_index` is identified, resolved from the
/// live `Inventory` (the server's `sell_items` snapshot carries no identify
/// state). A slot that's vanished from the bag (stale snapshot) defaults to
/// `false` — the conservative choice, since defaulting to `true` is exactly the
/// name/icon leak this guards against.
fn sell_identified(inventory: &Inventory, index: u32) -> bool {
    inventory
        .get(index as u16)
        .map(|item| item.identified)
        .unwrap_or(false)
}

/// One entry per card slot the item type has, `true` where that slot holds a
/// card — mirrors `inventory_window`'s `spawn_card_slots` recipe. Resolved from
/// the live `Inventory` by `inventory_index` (the sell snapshot carries no card
/// data); empty when the slot is gone or the item type has no sockets.
fn sell_cards(inventory: &Inventory, item_db: Option<&ItemDb>, index: u32) -> Vec<bool> {
    let Some(item) = inventory.get(index as u16) else {
        return Vec::new();
    };
    let slots = item_db
        .and_then(|db| db.slot_count(item.item_id))
        .unwrap_or(0);
    (0..slots)
        .map(|slot| item.cards.get(slot as usize).copied().unwrap_or(0) != 0)
        .collect()
}

/// The Sell cell's refine badge, resolved from the live `Inventory` by
/// `inventory_index` (the server's `sell_items` snapshot carries no refine data
/// — design §8). `None` for an unrefined or no-longer-present slot.
fn sell_refine(inventory: &Inventory, index: u32) -> Option<u8> {
    inventory
        .get(index as u16)
        .map(|item| item.refine)
        .filter(|refine| *refine > 0)
}

/// Whether the active tab's cart is non-empty and, on the Buy tab, affordable.
/// Reused by [`on_shop_button`](super::on_shop_button) as the `OpenConfirm`
/// guard, since the CTA's disabled look is visual-only (no `InteractionDisabled`).
pub(super) fn cta_enabled(session: &ShopSession, zeny: u32) -> bool {
    let cart_empty = match session.tab {
        ShopTab::Buy => session.cart_buy.is_empty(),
        ShopTab::Sell => session.cart_sell.is_empty(),
    };
    if cart_empty {
        return false;
    }
    match session.tab {
        ShopTab::Buy => session.can_afford(zeny),
        ShopTab::Sell => true,
    }
}

/// The footer's "not enough zeny" state: only the Buy tab can ever be
/// unaffordable (a sell cart always earns zeny, never spends it).
fn footer_warning(session: &ShopSession, zeny: u32) -> bool {
    session.tab == ShopTab::Buy && !session.can_afford(zeny)
}

fn row_views(
    session: &ShopSession,
    item_db: Option<&ItemDb>,
    inventory: &Inventory,
) -> Vec<RowView> {
    match session.tab {
        ShopTab::Buy => session
            .buy_items
            .iter()
            .map(|item| RowView {
                icon: icon_path(item_db, item.nameid, true),
                name: item_name(item_db, item.nameid, true),
                price: item.price,
                refine: None,
                owned: None,
                cart_qty: session
                    .cart_buy
                    .get(&item.nameid)
                    .copied()
                    .filter(|q| *q > 0),
                selected: session.selected == Some(Selection::Buy(item.nameid)),
                action: ShopButtonAction::Select(Selection::Buy(item.nameid)),
            })
            .collect(),
        ShopTab::Sell => session
            .sell_items
            .iter()
            .map(|item| {
                let identified = sell_identified(inventory, item.inventory_index);
                RowView {
                    icon: icon_path(item_db, item.nameid, identified),
                    name: item_name(item_db, item.nameid, identified),
                    price: item.sell_price,
                    refine: sell_refine(inventory, item.inventory_index),
                    owned: Some(item.amount),
                    cart_qty: session
                        .cart_sell
                        .get(&item.inventory_index)
                        .copied()
                        .filter(|q| *q > 0),
                    selected: session.selected == Some(Selection::Sell(item.inventory_index)),
                    action: ShopButtonAction::Select(Selection::Sell(item.inventory_index)),
                }
            })
            .collect(),
    }
}

fn detail_view(
    session: &ShopSession,
    item_db: Option<&ItemDb>,
    inventory: &Inventory,
) -> Option<DetailView> {
    match session.selected? {
        Selection::Buy(nameid) => {
            let price = session
                .buy_items
                .iter()
                .find(|item| item.nameid == nameid)?
                .price;
            Some(DetailView {
                icon: icon_path(item_db, nameid, true),
                name: item_name(item_db, nameid, true),
                price,
                buy: true,
                amount: None,
                cards: Vec::new(),
                description: item_db
                    .and_then(|db| db.description(nameid, true))
                    .and_then(|lines| lines.first().cloned()),
                pending_qty: session.pending_qty,
            })
        }
        Selection::Sell(index) => {
            let item = session
                .sell_items
                .iter()
                .find(|item| item.inventory_index == index)?;
            let identified = sell_identified(inventory, index);
            Some(DetailView {
                icon: icon_path(item_db, item.nameid, identified),
                name: item_name(item_db, item.nameid, identified),
                price: item.sell_price,
                buy: false,
                amount: Some(item.amount),
                cards: sell_cards(inventory, item_db, index),
                description: item_db
                    .and_then(|db| db.description(item.nameid, identified))
                    .and_then(|lines| lines.first().cloned()),
                pending_qty: session.pending_qty,
            })
        }
    }
}

fn cart_lines(
    session: &ShopSession,
    item_db: Option<&ItemDb>,
    inventory: &Inventory,
) -> Vec<CartLineView> {
    match session.tab {
        ShopTab::Buy => session
            .buy_items
            .iter()
            .filter_map(|item| {
                let qty = *session.cart_buy.get(&item.nameid)?;
                (qty > 0).then(|| CartLineView {
                    key: item.nameid,
                    name: item_name(item_db, item.nameid, true),
                    unit_price: item.price,
                    qty,
                })
            })
            .collect(),
        ShopTab::Sell => session
            .sell_items
            .iter()
            .filter_map(|item| {
                let qty = *session.cart_sell.get(&item.inventory_index)?;
                let identified = sell_identified(inventory, item.inventory_index);
                (qty > 0).then(|| CartLineView {
                    key: item.inventory_index,
                    name: item_name(item_db, item.nameid, identified),
                    unit_price: item.sell_price,
                    qty,
                })
            })
            .collect(),
    }
}

/// Total quantity across every line of the active tab's cart — not the line
/// *count* — since the CTA badge sums quantities (e.g. "Buy · 53").
fn cart_qty_total(session: &ShopSession) -> u32 {
    match session.tab {
        ShopTab::Buy => session.cart_buy.values().sum(),
        ShopTab::Sell => session.cart_sell.values().sum(),
    }
}

/// The CTA's label: the bare verb when the cart is empty, otherwise
/// `"{verb} · {qty}"` (e.g. "Sell · 12").
fn cta_label(buy: bool, cart_qty: u32) -> String {
    let verb = if buy { "Buy" } else { "Sell" };
    if cart_qty > 0 {
        format!("{verb} \u{b7} {cart_qty}")
    } else {
        verb.to_string()
    }
}

fn footer_view(session: &ShopSession, zeny: u32) -> FooterView {
    let buy = session.tab == ShopTab::Buy;
    let total = if buy {
        session.buy_subtotal()
    } else {
        session.sell_subtotal()
    };
    FooterView {
        zeny,
        total,
        buy,
        warning: footer_warning(session, zeny),
        cta_label: cta_label(buy, cart_qty_total(session)),
        cta_enabled: cta_enabled(session, zeny),
    }
}

/// The whole swappable body: tab strip, item grid + detail/cart side column, and
/// footer. Spawned as a single child of [`ShopWindowBody`](super::ShopWindowBody)
/// by `rebuild_body` on every `ShopSession` change.
pub fn body(
    session: &ShopSession,
    zeny: u32,
    item_db: Option<&ItemDb>,
    inventory: &Inventory,
) -> impl Scene {
    let tab = session.tab;
    let buy = tab == ShopTab::Buy;
    let rows = row_views(session, item_db, inventory);
    let detail = detail_view(session, item_db, inventory);
    let lines = cart_lines(session, item_db, inventory);
    let total = if buy {
        session.buy_subtotal()
    } else {
        session.sell_subtotal()
    };
    let footer_data = footer_view(session, zeny);
    let overlay = session
        .confirm_open
        .then(|| EntityScene(confirm_overlay(buy, lines.clone(), total)));
    let cart = CartView { buy, lines, total };

    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(12),
        }
        ignore_picking()
        Children [ tab_strip(tab), content_row(tab, rows, detail, cart), footer(footer_data), {overlay} ]
    }
}

fn tab_strip(tab: ShopTab) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Row, column_gap: px(6) }
        ignore_picking()
        Children [
            tab_button("Buy", ShopTab::Buy, tab == ShopTab::Buy),
            tab_button("Sell", ShopTab::Sell, tab == ShopTab::Sell),
        ]
    }
}

fn tab_button(label: &'static str, target: ShopTab, active: bool) -> impl Scene {
    let bg = if active { theme::EMERALD } else { theme::FIELD };
    bsn! {
        @FeathersButton { @caption: bsn! { chrome_text(label.to_string()) } }
        template_value(ShopButtonAction::SwitchTab(target))
        Node { flex_grow: 1.0, height: px(26) }
        BackgroundColor(bg)
        on(on_shop_button)
    }
}

fn content_row(
    tab: ShopTab,
    rows: Vec<RowView>,
    detail: Option<DetailView>,
    cart: CartView,
) -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            column_gap: px(12),
        }
        ignore_picking()
        Children [ grid_pane(tab, rows), side_column(tab, detail, cart) ]
    }
}

/// The bordered, rounded stock pane: a header (label + item count) over the
/// scrollable item list. Fixed height so the window never grows; the list
/// scrolls internally. Mirrors the mockup's `.sh-grid-pane`.
fn grid_pane(tab: ShopTab, rows: Vec<RowView>) -> impl Scene {
    let label = if tab == ShopTab::Buy {
        "For Sale"
    } else {
        "Your Goods"
    };
    let unit = if tab == ShopTab::Buy {
        "wares"
    } else {
        "stacks"
    };
    let count = format!("{} {unit}", rows.len());
    bsn! {
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            min_width: px(0),
            height: px(PANE_HEIGHT),
            flex_direction: FlexDirection::Column,
            border: px(1),
            border_radius: BorderRadius::all(px(12)),
        }
        BackgroundColor({Color::srgba(0.0, 0.0, 0.0, 0.22)})
        BorderColor::all(theme::STROKE)
        ignore_picking()
        Children [ pane_head(label.to_string(), count), stock_list(rows) ]
    }
}

fn pane_head(label: String, count: String) -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            padding: {UiRect::axes(px(12), px(9))},
            border: {UiRect { bottom: Val::Px(1.0), ..default() }},
        }
        BorderColor::all(theme::STROKE)
        ignore_picking()
        Children [
            (
                Text(label)
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/cinzel.ttf"),
                    font_size: {FontSize::Px(13.0)},
                }
                ThemeTextColor({TOKEN_TEXT})
                ignore_picking()
            ),
            (Node { flex_grow: 1.0 } ignore_picking()),
            (
                Text(count)
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                    font_size: {FontSize::Px(10.0)},
                }
                ThemeTextColor({TOKEN_TEXT_DIM})
                ignore_picking()
            ),
        ]
    }
}

/// The scrollable stock list: a vertical column of [`stock_row`]s inside a
/// fixed-height, wheel-scrollable viewport (`ScrollArea`) with a draggable
/// [`FeathersScrollbar`] pinned to the right. The `#inner` id wires the
/// scrollbar to the viewport whose `ScrollPosition` it drives.
fn stock_list(rows: Vec<RowView>) -> impl Scene {
    let empty = rows.is_empty();
    let items: Vec<_> = rows.into_iter().map(stock_row).collect();
    let empty_msg = empty.then(|| EntityScene(muted_text("No items.".to_string())));
    bsn! {
        Node {
            flex_grow: 1.0,
            min_height: px(0),
            position_type: PositionType::Relative,
        }
        ignore_picking()
        Children [
            (
                #inner
                Node {
                    flex_grow: 1.0,
                    min_height: px(0),
                    overflow: {Overflow::scroll_y()},
                    flex_direction: FlexDirection::Column,
                    row_gap: px(5),
                    padding: {UiRect { left: Val::Px(9.0), right: Val::Px(12.0), top: Val::Px(9.0), bottom: Val::Px(9.0) }},
                }
                ScrollArea
                Pickable
                Children [ {items}, {empty_msg} ]
            ),
            @FeathersScrollbar { @target: #inner, @orientation: {ControlOrientation::Vertical} }
            Node {
                position_type: PositionType::Absolute,
                right: px(3),
                top: px(6),
                bottom: px(6),
                width: px(6),
            }
        ]
    }
}

/// One item row (mirrors the mockup's `.sh-line` recipe, applied to the stock
/// list): a sprite well on the left, the item name over an optional
/// refine/stock subtitle, an in-cart badge, and the unit price on the right.
/// The whole row is the select button.
fn stock_row(view: RowView) -> impl Scene {
    let (bg, border) = if view.selected {
        (theme::EMERALD_INK, theme::EMERALD)
    } else {
        (theme::FIELD, theme::STROKE)
    };
    let subtitle = row_subtitle(view.refine, view.owned).map(|text| EntityScene(row_sub(text)));
    let cart = view
        .cart_qty
        .map(|qty| EntityScene(cart_pill(qty.to_string())));
    let price_text = format!("{}z", view.price);
    let action = view.action;

    bsn! {
        @FeathersButton {
            @caption: bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(10),
                }
                ignore_picking()
                Children [
                    row_well(view.icon),
                    (
                        Node {
                            flex_grow: 1.0,
                            min_width: px(0),
                            flex_direction: FlexDirection::Column,
                            row_gap: px(2),
                        }
                        ignore_picking()
                        Children [ row_name(view.name), {subtitle} ]
                    ),
                    {cart},
                    row_price(price_text),
                ]
            }
        }
        template_value(action)
        Node {
            width: percent(100),
            height: px(52),
            padding: {UiRect::axes(px(9), px(0))},
            border: px(1),
            border_radius: BorderRadius::all(px(9)),
        }
        BackgroundColor(bg)
        BorderColor::all(border)
        on(on_shop_button)
    }
}

/// The row's sprite well: a fixed 44px square framing a contained item icon.
fn row_well(icon: Option<String>) -> impl Scene {
    let inner = icon.map(|path| EntityScene(cell_icon(path)));
    bsn! {
        Node {
            width: px(44),
            height: px(44),
            flex_shrink: 0.0,
            position_type: PositionType::Relative,
            border: px(1),
            border_radius: BorderRadius::all(px(8)),
        }
        BackgroundColor({Color::srgba(0.0, 0.0, 0.0, 0.40)})
        BorderColor::all(theme::STROKE)
        ignore_picking()
        Children [ {inner} ]
    }
}

/// A contained item icon, centered in its parent at ~86% size — big enough to
/// read at a glance without touching the well's edges.
fn cell_icon(path: String) -> impl Scene {
    bsn! {
        ImageNode { image: {path} }
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            bottom: px(0),
            margin: {UiRect::all(Val::Auto)},
            width: percent(86),
            height: percent(86),
        }
        ignore_picking()
    }
}

/// The row's subtitle text (refine and/or stock count), or `None` when there's
/// nothing to say — the Buy tab, or an unrefined bag stack with no owned count.
fn row_subtitle(refine: Option<u8>, owned: Option<u32>) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(refine) = refine {
        parts.push(format!("+{refine}"));
    }
    if let Some(owned) = owned {
        parts.push(format!("{owned} owned"));
    }
    (!parts.is_empty()).then(|| parts.join(" \u{b7} "))
}

fn row_name(name: String) -> impl Scene {
    bsn! {
        Text(name)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(12.5)},
        }
        ThemeTextColor({TOKEN_TEXT})
        ignore_picking()
    }
}

fn row_sub(text: String) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(9.5)},
        }
        ThemeTextColor({TOKEN_TEXT_DIM})
        ignore_picking()
    }
}

/// The right-aligned unit price of a stock row.
fn row_price(text: String) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(12.0)},
        }
        ThemeTextColor({TOKEN_ACCENT})
        Node { flex_shrink: 0.0 }
        ignore_picking()
    }
}

/// The in-cart quantity pill shown on a row when the item is already staged in
/// the active cart.
fn cart_pill(text: String) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(9.5)},
        }
        TextColor(theme::EMERALD_INK)
        TextLayout { justify: Justify::Center }
        Node {
            flex_shrink: 0.0,
            min_width: px(18),
            padding: {UiRect::axes(px(5), px(2))},
            border_radius: BorderRadius::all(px(5)),
        }
        BackgroundColor(theme::EMERALD)
        ignore_picking()
    }
}

/// One "◆" pip per card slot, emerald when filled and faint when empty —
/// mirrors `inventory_window`'s `spawn_card_slots` recipe.
fn card_row(cards: Vec<bool>) -> impl Scene {
    let pips: Vec<_> = cards.into_iter().map(card_pip).collect();
    bsn! {
        Node { flex_direction: FlexDirection::Row, column_gap: px(2) }
        ignore_picking()
        Children [ {pips} ]
    }
}

fn card_pip(filled: bool) -> impl Scene {
    let color = if filled {
        theme::EMERALD
    } else {
        theme::TEXT_FAINT
    };
    bsn! {
        Text({"\u{25C6}".to_string()})
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(9.0)},
        }
        TextColor(color)
        ignore_picking()
    }
}

/// The right side column: one bordered, rounded container holding the detail
/// block over the basket block. Mirrors the mockup's `.sh-side-inner`.
fn side_column(tab: ShopTab, detail: Option<DetailView>, cart: CartView) -> impl Scene {
    bsn! {
        Node {
            width: px(SIDE_COLUMN_WIDTH),
            height: px(PANE_HEIGHT),
            flex_direction: FlexDirection::Column,
            border: px(1),
            border_radius: BorderRadius::all(px(12)),
        }
        BackgroundColor({Color::srgba(0.0, 0.0, 0.0, 0.22)})
        BorderColor::all(theme::STROKE)
        ignore_picking()
        Children [ detail_panel(tab, detail), cart_panel(cart) ]
    }
}

fn detail_panel(tab: ShopTab, detail: Option<DetailView>) -> impl Scene {
    let empty = detail.is_none();
    let filled = detail.map(|view| EntityScene(detail_content(view)));
    let empty_label = if tab == ShopTab::Buy {
        "Select an item to inspect & buy it"
    } else {
        "Select an item to inspect & sell it"
    };
    let empty_msg = empty.then(|| EntityScene(muted_text(empty_label.to_string())));
    bsn! {
        Node {
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            row_gap: px(8),
            padding: {UiRect::all(px(14))},
            border: {UiRect { bottom: Val::Px(1.0), ..default() }},
        }
        BorderColor::all(theme::STROKE)
        ignore_picking()
        Children [ {filled}, {empty_msg} ]
    }
}

/// The filled detail block: a header (sprite well + title), a meta row of two
/// stat boxes (price, stock/owned), the description, and the add row
/// (quantity stepper + Add to Cart/Sale). Mirrors the mockup's `.sh-detail`.
fn detail_content(view: DetailView) -> impl Scene {
    let icon = view.icon.map(|path| EntityScene(cell_icon(path)));
    let price_label = if view.buy { "Unit price" } else { "Sell price" };
    let stock_label = if view.buy { "In stock" } else { "You own" };
    let stock_value = if view.buy {
        "Unlimited".to_string()
    } else {
        view.amount.unwrap_or(0).to_string()
    };
    let cards = (!view.cards.is_empty()).then(|| EntityScene(card_row(view.cards)));
    let description = view.description.map(|text| EntityScene(muted_text(text)));
    let add_label = if view.buy {
        "Add to Cart"
    } else {
        "Add to Sale"
    }
    .to_string();
    let add_bg = if view.buy {
        theme::EMERALD
    } else {
        theme::GOLD
    };

    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(8) }
        ignore_picking()
        Children [
            (
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: px(11),
                    align_items: AlignItems::FlexStart,
                }
                ignore_picking()
                Children [
                    (
                        Node {
                            width: px(50),
                            height: px(50),
                            flex_shrink: 0.0,
                            position_type: PositionType::Relative,
                            border: px(1),
                            border_radius: BorderRadius::all(px(9)),
                        }
                        BackgroundColor({Color::srgba(0.0, 0.0, 0.0, 0.42)})
                        BorderColor::all(theme::STROKE)
                        ignore_picking()
                        Children [ {icon} ]
                    ),
                    (
                        Text({view.name})
                        TextFont {
                            font: FontSourceTemplate::Handle("fonts/cinzel.ttf"),
                            font_size: {FontSize::Px(14.0)},
                        }
                        ThemeTextColor({TOKEN_TEXT})
                        Node { flex_grow: 1.0 }
                        ignore_picking()
                    ),
                ]
            ),
            (
                Node { flex_direction: FlexDirection::Row, column_gap: px(8) }
                ignore_picking()
                Children [
                    stat_box(price_label.to_string(), format!("{}z", view.price)),
                    stat_box(stock_label.to_string(), stock_value),
                ]
            ),
            {cards},
            {description},
            add_row(view.pending_qty, add_label, add_bg),
        ]
    }
}

/// One stat box of the detail's meta row (mirrors `.sh-d-stat`): a small
/// uppercase label over a bold gold-accented value.
fn stat_box(label: String, value: String) -> impl Scene {
    bsn! {
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            flex_direction: FlexDirection::Column,
            row_gap: px(3),
            padding: {UiRect::axes(px(10), px(8))},
            border: px(1),
            border_radius: BorderRadius::all(px(8)),
        }
        BackgroundColor({Color::srgba(0.0, 0.0, 0.0, 0.26)})
        BorderColor::all(theme::STROKE)
        ignore_picking()
        Children [
            (
                Text(label)
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                    font_size: {FontSize::Px(8.5)},
                }
                ThemeTextColor({TOKEN_TEXT_DIM})
                ignore_picking()
            ),
            (
                Text(value)
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                    font_size: {FontSize::Px(13.0)},
                }
                ThemeTextColor({TOKEN_ACCENT})
                ignore_picking()
            ),
        ]
    }
}

/// The detail's add row: the pending-quantity stepper plus the Add to
/// Cart/Sale button. Mirrors the mockup's `.sh-d-add`.
fn add_row(qty: u32, label: String, bg: Color) -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(8),
        }
        ignore_picking()
        Children [ stepper(qty), add_to_cart_button(label, bg) ]
    }
}

/// The pending-quantity stepper (mirrors `.sh-stepper`): minus / value / plus,
/// driving `ShopSession.pending_qty` via `PendingDec`/`PendingInc`.
fn stepper(qty: u32) -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Stretch,
            height: px(30),
            border: px(1),
            border_radius: BorderRadius::all(px(7)),
        }
        BackgroundColor(theme::FIELD)
        BorderColor::all(theme::STROKE)
        ignore_picking()
        Children [
            stepper_button("minus", ShopButtonAction::PendingDec),
            stepper_value(qty),
            stepper_button("plus", ShopButtonAction::PendingInc),
        ]
    }
}

fn stepper_button(icon_name: &'static str, action: ShopButtonAction) -> impl Scene {
    bsn! {
        @FeathersButton { @caption: bsn! { glyph_icon(icon_name, 12.0, theme::TEXT_DIM) } }
        template_value(action)
        Node { width: px(26), height: px(28) }
        on(on_shop_button)
    }
}

fn stepper_value(qty: u32) -> impl Scene {
    bsn! {
        Text({qty.to_string()})
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(12.0)},
        }
        ThemeTextColor({TOKEN_TEXT})
        TextLayout { justify: Justify::Center }
        Node {
            width: px(36),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
        }
        ignore_picking()
    }
}

fn add_to_cart_button(label: String, bg: Color) -> impl Scene {
    bsn! {
        @FeathersButton { @caption: bsn! { chrome_text(label) } }
        template_value(ShopButtonAction::AddToCart)
        Node {
            flex_grow: 1.0,
            height: px(30),
            padding: {UiRect::horizontal(px(10))},
        }
        BackgroundColor(bg)
        on(on_shop_button)
    }
}

fn meta_row(label: String, value: String) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Row, justify_content: JustifyContent::SpaceBetween }
        ignore_picking()
        Children [
            (
                Text(label)
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                    font_size: {FontSize::Px(10.5)},
                }
                ThemeTextColor({TOKEN_TEXT_DIM})
                ignore_picking()
            ),
            (
                Text(value)
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                    font_size: {FontSize::Px(11.0)},
                }
                ThemeTextColor({TOKEN_TEXT})
                ignore_picking()
            ),
        ]
    }
}

fn muted_text(text: String) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(11.0)},
        }
        ThemeTextColor({TOKEN_TEXT_DIM})
        ignore_picking()
    }
}

/// The basket block: a header ("Cart · N items"/"To Sell · N items"), the
/// cart lines, and a subtotal row pinned at the bottom. Mirrors the mockup's
/// `.sh-basket`.
fn cart_panel(view: CartView) -> impl Scene {
    let empty = view.lines.is_empty();
    let count = view.lines.len();
    let header_label = if view.buy { "Cart" } else { "To Sell" };
    let unit = if count == 1 { "item" } else { "items" };
    let header = format!("{header_label} \u{b7} {count} {unit}");
    let subtotal_label = if view.buy { "Subtotal" } else { "You receive" }.to_string();
    let total = view.total;
    let rows: Vec<_> = view.lines.into_iter().map(cart_line).collect();
    let empty_msg = empty.then(|| EntityScene(muted_text("Nothing added yet.".to_string())));
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            min_height: px(0),
        }
        ignore_picking()
        Children [
            basket_head(header),
            (
                Node {
                    flex_grow: 1.0,
                    min_height: px(0),
                    position_type: PositionType::Relative,
                }
                ignore_picking()
                Children [
                    (
                        #basket
                        Node {
                            flex_grow: 1.0,
                            min_height: px(0),
                            overflow: {Overflow::scroll_y()},
                            flex_direction: FlexDirection::Column,
                            row_gap: px(6),
                            padding: {UiRect { left: Val::Px(11.0), right: Val::Px(13.0), top: Val::Px(2.0), bottom: Val::Px(4.0) }},
                        }
                        ScrollArea
                        Pickable
                        Children [ {rows}, {empty_msg} ]
                    ),
                    @FeathersScrollbar { @target: #basket, @orientation: {ControlOrientation::Vertical} }
                    Node {
                        position_type: PositionType::Absolute,
                        right: px(2),
                        top: px(2),
                        bottom: px(2),
                        width: px(5),
                    }
                ]
            ),
            subtotal_row(subtotal_label, total),
        ]
    }
}

fn basket_head(text: String) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(9.5)},
        }
        ThemeTextColor({TOKEN_TEXT_DIM})
        Node { padding: {UiRect::axes(px(13), px(8))} }
        ignore_picking()
    }
}

fn subtotal_row(label: String, total: u64) -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            padding: {UiRect::axes(px(13), px(9))},
            border: {UiRect { top: Val::Px(1.0), ..default() }},
        }
        BorderColor::all(theme::STROKE)
        ignore_picking()
        Children [
            (
                Text(label)
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                    font_size: {FontSize::Px(9.5)},
                }
                ThemeTextColor({TOKEN_TEXT_DIM})
                ignore_picking()
            ),
            (
                Text({format!("{total}z")})
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                    font_size: {FontSize::Px(14.0)},
                }
                ThemeTextColor({TOKEN_ACCENT})
                ignore_picking()
            ),
        ]
    }
}

fn cart_line(view: CartLineView) -> impl Scene {
    let total = view.unit_price as u64 * view.qty as u64;
    let key = view.key;
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(4),
            padding: {UiRect::all(px(6))},
            border: px(1),
            border_radius: BorderRadius::all(px(7)),
        }
        BackgroundColor({Color::srgba(0.0, 0.0, 0.0, 0.26)})
        BorderColor::all(theme::STROKE)
        ignore_picking()
        Children [
            (
                Text({view.name})
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                    font_size: {FontSize::Px(11.0)},
                }
                ThemeTextColor({TOKEN_TEXT})
                Node { flex_grow: 1.0 }
                ignore_picking()
            ),
            small_icon_button("minus", ShopButtonAction::DecQty(key)),
            (
                Text({view.qty.to_string()})
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                    font_size: {FontSize::Px(11.0)},
                }
                ThemeTextColor({TOKEN_TEXT})
                ignore_picking()
            ),
            small_icon_button("plus", ShopButtonAction::IncQty(key)),
            small_icon_button("trash", ShopButtonAction::RemoveLine(key)),
            (
                Text({format!("{total}z")})
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                    font_size: {FontSize::Px(11.0)},
                }
                ThemeTextColor({TOKEN_ACCENT})
                ignore_picking()
            ),
        ]
    }
}

fn small_icon_button(icon_name: &'static str, action: ShopButtonAction) -> impl Scene {
    bsn! {
        @FeathersButton { @caption: bsn! { glyph_icon(icon_name, 10.0, theme::TEXT_DIM) } }
        template_value(action)
        Node { width: px(18), height: px(16) }
        on(on_shop_button)
    }
}

fn footer(view: FooterView) -> impl Scene {
    let warn = view.warning.then(|| EntityScene(warning_text()));
    let total = (!view.warning).then(|| EntityScene(total_text(view.buy, view.total)));

    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(10),
            padding: {UiRect::axes(px(14), px(10))},
            border: {UiRect { top: Val::Px(1.0), ..default() }},
        }
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        ignore_picking()
        Children [
            zeny_display(view.zeny),
            (Node { flex_grow: 1.0 } ignore_picking()),
            {warn},
            {total},
            cta_button(view.buy, view.cta_label, view.cta_enabled),
        ]
    }
}

/// The footer's zeny display: a coin glyph beside a stacked "YOUR ZENY"
/// label over the gold balance. Mirrors the mockup's `.sh-zeny`.
fn zeny_display(zeny: u32) -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(8),
        }
        ignore_picking()
        Children [
            glyph_icon("coin", 16.0, theme::GOLD),
            (
                Node { flex_direction: FlexDirection::Column, row_gap: px(1) }
                ignore_picking()
                Children [
                    (
                        Text({"YOUR ZENY".to_string()})
                        TextFont {
                            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                            font_size: {FontSize::Px(9.0)},
                        }
                        ThemeTextColor({TOKEN_TEXT_DIM})
                        ignore_picking()
                    ),
                    (
                        Text({format!("{zeny}z")})
                        TextFont {
                            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                            font_size: {FontSize::Px(13.0)},
                        }
                        TextColor(theme::GOLD)
                        ignore_picking()
                    ),
                ]
            ),
        ]
    }
}

fn warning_text() -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(4),
        }
        ignore_picking()
        Children [
            glyph_icon("warn", 12.0, theme::BAD),
            (
                Text({"Not enough zeny".to_string()})
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                    font_size: {FontSize::Px(12.0)},
                }
                TextColor(theme::BAD)
                ignore_picking()
            ),
        ]
    }
}

fn total_text(buy: bool, total: u64) -> impl Scene {
    let sign = if total == 0 {
        ""
    } else if buy {
        "-"
    } else {
        "+"
    };
    let label = if buy { "Total" } else { "You receive" };
    let color = if buy { theme::TEXT } else { theme::EMERALD };
    bsn! {
        Node { flex_direction: FlexDirection::Column, align_items: AlignItems::FlexEnd }
        ignore_picking()
        Children [
            (
                Text({label.to_string()})
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                    font_size: {FontSize::Px(10.0)},
                }
                ThemeTextColor({TOKEN_TEXT_DIM})
                ignore_picking()
            ),
            (
                Text({format!("{sign}{total}z")})
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                    font_size: {FontSize::Px(13.0)},
                }
                TextColor(color)
                ignore_picking()
            ),
        ]
    }
}

fn cta_button(buy: bool, label: String, enabled: bool) -> impl Scene {
    let bg = if enabled {
        if buy {
            theme::EMERALD
        } else {
            theme::GOLD
        }
    } else {
        theme::FIELD
    };
    bsn! {
        @FeathersButton { @caption: bsn! { chrome_text(label) } }
        template_value(ShopButtonAction::OpenConfirm)
        Node { height: px(30), padding: {UiRect::horizontal(px(16))} }
        BackgroundColor(bg)
        on(on_shop_button)
    }
}

fn chrome_text(text: String) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(11.0)},
        }
        ThemeTextColor({TOKEN_TEXT_DIM})
        ignore_picking()
    }
}

// ---------------------------------------------------------------------------
// Confirm overlay: a dimmed backdrop over the whole body region, blocking
// clicks to the grid/cart/footer beneath while it's up (design §5.4). Rendered
// as part of `body()` whenever `ShopSession.confirm_open` is set — no separate
// spawn/despawn path, it just rides `rebuild_body`'s existing rebuild.
// ---------------------------------------------------------------------------

fn confirm_overlay(buy: bool, lines: Vec<CartLineView>, total: u64) -> impl Scene {
    let title = if buy {
        "Confirm Purchase"
    } else {
        "Confirm Sale"
    };
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
        }
        BackgroundColor({Color::srgba(0.0, 0.0, 0.0, 0.55)})
        Pickable
        Children [ confirm_card(title.to_string(), lines, total, buy) ]
    }
}

fn confirm_card(title: String, lines: Vec<CartLineView>, total: u64, buy: bool) -> impl Scene {
    let rows: Vec<_> = lines.into_iter().map(confirm_line).collect();
    let total_label = if buy { "Total cost" } else { "Total payout" };
    let sign = if buy { "-" } else { "+" };
    let confirm_label = if buy {
        "Confirm Purchase"
    } else {
        "Confirm Sale"
    };
    let confirm_bg = if buy { theme::EMERALD } else { theme::GOLD };

    bsn! {
        Node {
            width: px(280),
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
            padding: {UiRect::all(px(16))},
            border: px(1),
            border_radius: BorderRadius::all(px(10)),
        }
        BackgroundColor(theme::GLASS)
        BorderColor::all(theme::GOLD_FAINT)
        Pickable
        Children [
            (
                Text(title)
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/cinzel.ttf"),
                    font_size: {FontSize::Px(15.0)},
                }
                ThemeTextColor({TOKEN_TEXT})
                ignore_picking()
            ),
            (
                Node { flex_direction: FlexDirection::Column, row_gap: px(4) }
                ignore_picking()
                Children [ {rows} ]
            ),
            meta_row(total_label.to_string(), format!("{sign}{total}z")),
            confirm_actions(confirm_label.to_string(), confirm_bg),
        ]
    }
}

fn confirm_line(view: CartLineView) -> impl Scene {
    let total = view.unit_price as u64 * view.qty as u64;
    bsn! {
        Node { flex_direction: FlexDirection::Row, justify_content: JustifyContent::SpaceBetween }
        ignore_picking()
        Children [
            (
                Text({format!("{} x{}", view.name, view.qty)})
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                    font_size: {FontSize::Px(11.0)},
                }
                ThemeTextColor({TOKEN_TEXT})
                ignore_picking()
            ),
            (
                Text({format!("{total}z")})
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                    font_size: {FontSize::Px(11.0)},
                }
                ThemeTextColor({TOKEN_ACCENT})
                ignore_picking()
            ),
        ]
    }
}

fn confirm_actions(confirm_label: String, confirm_bg: Color) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Row, column_gap: px(8) }
        ignore_picking()
        Children [
            (
                @FeathersButton { @caption: bsn! { chrome_text("Cancel".to_string()) } }
                template_value(ShopButtonAction::CancelConfirm)
                Node { flex_grow: 1.0, height: px(28) }
                on(on_shop_button)
            ),
            (
                @FeathersButton { @caption: bsn! { chrome_text(confirm_label) } }
                template_value(ShopButtonAction::ConfirmTrade)
                Node { flex_grow: 1.0, height: px(28) }
                BackgroundColor(confirm_bg)
                on(on_shop_button)
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use game_engine::domain::inventory::Item;
    use net_contract::dto::{ShopBuyItem, ShopSellItem};

    use super::*;

    fn session_with(tab: ShopTab) -> ShopSession {
        ShopSession {
            unit_id: 1,
            buy_items: vec![ShopBuyItem {
                nameid: 501,
                price: 50,
            }],
            sell_items: vec![ShopSellItem {
                inventory_index: 0,
                nameid: 501,
                amount: 5,
                sell_price: 20,
            }],
            tab,
            cart_buy: HashMap::new(),
            cart_sell: HashMap::new(),
            selected: None,
            pending_qty: 1,
            banner: None,
            confirm_open: false,
            awaiting: false,
        }
    }

    #[test]
    fn cta_disabled_when_cart_empty() {
        let session = session_with(ShopTab::Buy);
        assert!(!cta_enabled(&session, 1000));
    }

    #[test]
    fn cta_enabled_when_buy_cart_affordable() {
        let mut session = session_with(ShopTab::Buy);
        session.cart_buy.insert(501, 2);
        assert!(cta_enabled(&session, 100));
    }

    #[test]
    fn cta_disabled_when_buy_cart_unaffordable() {
        let mut session = session_with(ShopTab::Buy);
        session.cart_buy.insert(501, 3);
        assert!(!cta_enabled(&session, 100));
    }

    #[test]
    fn cta_enabled_on_sell_regardless_of_zeny() {
        let mut session = session_with(ShopTab::Sell);
        session.cart_sell.insert(0, 5);
        assert!(cta_enabled(&session, 0));
    }

    #[test]
    fn cart_qty_total_sums_active_tab_only() {
        let mut session = session_with(ShopTab::Buy);
        session.cart_buy.insert(501, 3);
        session.cart_sell.insert(0, 5);
        assert_eq!(cart_qty_total(&session), 3);
    }

    #[test]
    fn cta_label_omits_count_when_cart_empty() {
        assert_eq!(cta_label(true, 0), "Buy");
        assert_eq!(cta_label(false, 0), "Sell");
    }

    #[test]
    fn cta_label_includes_cart_count() {
        assert_eq!(cta_label(true, 30), "Buy \u{b7} 30");
        assert_eq!(cta_label(false, 7), "Sell \u{b7} 7");
    }

    #[test]
    fn footer_warning_only_on_unaffordable_buy() {
        let mut session = session_with(ShopTab::Buy);
        session.cart_buy.insert(501, 3);
        assert!(footer_warning(&session, 100));
        assert!(!footer_warning(&session, 1000));
    }

    #[test]
    fn footer_warning_never_shows_on_sell() {
        let mut session = session_with(ShopTab::Sell);
        session.cart_sell.insert(0, 5);
        assert!(!footer_warning(&session, 0));
    }

    #[test]
    fn row_subtitle_none_on_buy() {
        assert_eq!(row_subtitle(None, None), None);
    }

    #[test]
    fn row_subtitle_shows_owned_on_sell() {
        assert_eq!(row_subtitle(None, Some(10)), Some("10 owned".to_string()));
    }

    #[test]
    fn row_subtitle_combines_refine_and_owned() {
        assert_eq!(
            row_subtitle(Some(7), Some(3)),
            Some("+7 \u{b7} 3 owned".to_string())
        );
    }

    #[test]
    fn sell_refine_omitted_when_zero_or_absent() {
        let mut inv = Inventory::default();
        inv.upsert(Item {
            index: 0,
            refine: 0,
            ..Default::default()
        });
        assert_eq!(sell_refine(&inv, 0), None);
        assert_eq!(sell_refine(&inv, 99), None);
    }

    #[test]
    fn sell_refine_present_when_positive() {
        let mut inv = Inventory::default();
        inv.upsert(Item {
            index: 0,
            refine: 4,
            ..Default::default()
        });
        assert_eq!(sell_refine(&inv, 0), Some(4));
    }

    #[test]
    fn sell_identified_reflects_the_slot() {
        let mut inv = Inventory::default();
        inv.upsert(Item {
            index: 0,
            identified: true,
            ..Default::default()
        });
        inv.upsert(Item {
            index: 1,
            identified: false,
            ..Default::default()
        });
        assert!(sell_identified(&inv, 0));
        assert!(!sell_identified(&inv, 1));
    }

    #[test]
    fn sell_identified_defaults_false_when_slot_absent() {
        let inv = Inventory::default();
        assert!(!sell_identified(&inv, 99));
    }

    fn card_item(index: u16, cards: [u32; 4]) -> Item {
        Item {
            index,
            item_id: 501,
            cards,
            ..Default::default()
        }
    }

    fn db_with_slots(slots: u8) -> ItemDb {
        use lifthrasir_data::{ItemData, ItemInfo};
        let mut data = ItemData::default();
        data.items.insert(
            501,
            ItemInfo {
                slot_count: slots,
                ..Default::default()
            },
        );
        ItemDb::from_item_data(data)
    }

    #[test]
    fn sell_cards_reflects_filled_and_empty_slots() {
        let mut inv = Inventory::default();
        inv.upsert(card_item(0, [111, 0, 222, 0]));
        let db = db_with_slots(4);
        assert_eq!(
            sell_cards(&inv, Some(&db), 0),
            vec![true, false, true, false]
        );
    }

    #[test]
    fn sell_cards_empty_when_item_type_has_no_slots() {
        let mut inv = Inventory::default();
        inv.upsert(card_item(0, [111, 0, 0, 0]));
        let db = db_with_slots(0);
        assert_eq!(sell_cards(&inv, Some(&db), 0), Vec::<bool>::new());
    }

    #[test]
    fn sell_cards_empty_when_slot_absent() {
        let db = db_with_slots(4);
        assert_eq!(
            sell_cards(&Inventory::default(), Some(&db), 99),
            Vec::<bool>::new()
        );
    }
}
