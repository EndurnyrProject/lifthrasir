//! Idiomatic BSN chrome for the NPC shop window: a draggable, single-instance
//! window (design `2026-07-07-npc-shops` §5.4). [`window`] builds the whole chrome
//! — root, titlebar, and an (initially empty) body — as one `bsn!` tree; Task 7
//! fills the body region.

use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy_feathers::controls::FeathersButton;
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor, ThemeTextColor};
use game_engine::domain::assets::item_icon_path;
use game_engine::domain::inventory::Inventory;
use game_engine::infrastructure::item::ItemDb;

use crate::theme;
use crate::theme::feathers_theme::{
    TOKEN_ACCENT, TOKEN_TEXT, TOKEN_TEXT_DIM, TOKEN_TITLEBAR_BG, TOKEN_WINDOW_BG,
    TOKEN_WINDOW_BORDER,
};
use crate::widgets::draggable::px_or_zero;

use super::{
    on_shop_close_button, Selection, ShopButtonAction, ShopSession, ShopTab, ShopWindowBody,
    ShopWindowRoot, ShopWindowTitlebar,
};

const WINDOW_LEFT: f32 = 340.0;
const WINDOW_TOP: f32 = 90.0;
const WINDOW_WIDTH: f32 = 420.0;

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
        Children [ titlebar(title), body_container() ]
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
        on(on_titlebar_drag)
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

/// The (initially empty) body region; [`rebuild_body`](super::rebuild_body) fills
/// it with [`body`]'s content on every `ShopSession` change.
fn body_container() -> impl Scene {
    bsn! {
        ShopWindowBody
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
            padding: {UiRect::axes(px(14), px(12))},
        }
        ignore_picking()
    }
}

/// Drags the single open shop window by its titlebar; mirrors `make_draggable`
/// but resolves the root from its marker instead of a captured entity, so the
/// whole window can spawn as one scene with no imperative drag wiring. Only the
/// titlebar itself moves the window: `Pointer<Drag>` bubbles up from the close
/// button, so a drag targeting it is ignored.
fn on_titlebar_drag(
    drag: On<Pointer<Drag>>,
    titlebars: Query<(), With<ShopWindowTitlebar>>,
    mut roots: Query<&mut Node, With<ShopWindowRoot>>,
) {
    if titlebars.get(drag.entity).is_err() {
        return;
    }
    let Ok(mut node) = roots.single_mut() else {
        return;
    };
    node.left = Val::Px(px_or_zero(node.left) + drag.delta.x);
    node.top = Val::Px(px_or_zero(node.top) + drag.delta.y);
}

/// A square white SVG glyph tinted with `color`. `ImageNode` has no theme-token
/// tint, so glyph colors stay raw palette values.
fn glyph_icon(name: &'static str, size: f32, color: Color) -> impl Scene {
    bsn! {
        ImageNode {
            image: {format!("{}{}.svg", theme::ICON_DIR, name)},
            color: color,
        }
        Node { width: px(size), height: px(size) }
        ignore_picking()
    }
}

/// `Pickable::IGNORE` as a scene, so non-interactive nodes don't swallow clicks.
fn ignore_picking() -> impl Scene {
    bsn! {
        Pickable { should_block_lower: false, is_hoverable: false }
    }
}

// ---------------------------------------------------------------------------
// Body: tab strip, grid, detail panel, cart, footer (design §5.4).
//
// `bsn!` scenes own their data, so every view-model below is prepared as plain
// owned values (`String`/`u32`/...) *before* being handed to a `bsn!` block —
// no `ShopSession`/`ItemDb`/`Inventory` reference ever crosses into one.
// ---------------------------------------------------------------------------

const CELL_SIZE: f32 = 34.0;

struct CellView {
    icon: Option<String>,
    name: String,
    price: u32,
    badge: Option<String>,
    refine: Option<u8>,
    cards: Vec<bool>,
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
}

struct CartLineView {
    key: u32,
    name: String,
    unit_price: u32,
    qty: u32,
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

/// The cell's badge: an in-cart quantity always wins; otherwise the Sell tab
/// shows how many the player owns, and the Buy tab shows nothing (stock is
/// unbounded — design §9).
fn cell_badge(tab: ShopTab, cart_qty: Option<u32>, owned: Option<u32>) -> Option<String> {
    if let Some(qty) = cart_qty.filter(|qty| *qty > 0) {
        return Some(qty.to_string());
    }
    match tab {
        ShopTab::Sell => owned.map(|amount| amount.to_string()),
        ShopTab::Buy => None,
    }
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
fn cta_enabled(session: &ShopSession, zeny: u32) -> bool {
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

fn grid_cells(
    session: &ShopSession,
    item_db: Option<&ItemDb>,
    inventory: &Inventory,
) -> Vec<CellView> {
    match session.tab {
        ShopTab::Buy => session
            .buy_items
            .iter()
            .map(|item| {
                let cart_qty = session.cart_buy.get(&item.nameid).copied();
                CellView {
                    icon: icon_path(item_db, item.nameid, true),
                    name: item_name(item_db, item.nameid, true),
                    price: item.price,
                    badge: cell_badge(ShopTab::Buy, cart_qty, None),
                    refine: None,
                    cards: Vec::new(),
                    selected: session.selected == Some(Selection::Buy(item.nameid)),
                    action: ShopButtonAction::Select(Selection::Buy(item.nameid)),
                }
            })
            .collect(),
        ShopTab::Sell => session
            .sell_items
            .iter()
            .map(|item| {
                let cart_qty = session.cart_sell.get(&item.inventory_index).copied();
                let identified = sell_identified(inventory, item.inventory_index);
                CellView {
                    icon: icon_path(item_db, item.nameid, identified),
                    name: item_name(item_db, item.nameid, identified),
                    price: item.sell_price,
                    badge: cell_badge(ShopTab::Sell, cart_qty, Some(item.amount)),
                    refine: sell_refine(inventory, item.inventory_index),
                    cards: sell_cards(inventory, item_db, item.inventory_index),
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
        cta_label: if buy {
            "Buy".to_string()
        } else {
            "Sell".to_string()
        },
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
    let cells = grid_cells(session, item_db, inventory);
    let detail = detail_view(session, item_db, inventory);
    let cart = cart_lines(session, item_db, inventory);
    let footer_data = footer_view(session, zeny);

    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
        }
        ignore_picking()
        Children [ tab_strip(tab), content_row(cells, detail, cart), footer(footer_data) ]
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
    }
}

fn content_row(
    cells: Vec<CellView>,
    detail: Option<DetailView>,
    cart: Vec<CartLineView>,
) -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            column_gap: px(12),
            min_height: px(200),
        }
        ignore_picking()
        Children [ grid(cells), side_column(detail, cart) ]
    }
}

fn grid(cells: Vec<CellView>) -> impl Scene {
    let empty = cells.is_empty();
    let rows: Vec<_> = cells.into_iter().map(cell).collect();
    let empty_msg = empty.then(|| EntityScene(muted_text("No items.".to_string())));
    bsn! {
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            align_content: AlignContent::FlexStart,
            column_gap: px(6),
            row_gap: px(6),
        }
        ignore_picking()
        Children [ {rows}, {empty_msg} ]
    }
}

fn cell(view: CellView) -> impl Scene {
    let (bg, border) = if view.selected {
        (theme::EMERALD_INK, theme::EMERALD)
    } else {
        (theme::FIELD, theme::GOLD_FAINT)
    };
    let icon = view.icon.map(|path| EntityScene(cell_icon(path)));
    let refine = view
        .refine
        .map(|refine| EntityScene(corner_text(format!("+{refine}"), theme::GOLD, true)));
    let badge = view
        .badge
        .map(|badge| EntityScene(corner_text(badge, theme::TEXT, false)));
    let cards = (!view.cards.is_empty()).then(|| EntityScene(card_row(view.cards)));
    let price_text = format!("{}z", view.price);
    let name = view.name;
    let action = view.action;

    bsn! {
        @FeathersButton {
            @caption: bsn! {
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(2),
                }
                ignore_picking()
                Children [
                    (
                        Node {
                            width: px(CELL_SIZE),
                            height: px(CELL_SIZE),
                            position_type: PositionType::Relative,
                        }
                        ignore_picking()
                        Children [ {icon}, {refine}, {badge} ]
                    ),
                    cell_name(name),
                    {cards},
                    price_label(price_text),
                ]
            }
        }
        template_value(action)
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: {UiRect::all(px(4))},
            border: px(1),
            border_radius: BorderRadius::all(px(5)),
        }
        BackgroundColor(bg)
        BorderColor::all(border)
    }
}

fn cell_icon(path: String) -> impl Scene {
    bsn! {
        ImageNode { image: {path} }
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
        }
        ignore_picking()
    }
}

/// A small overlay label pinned to a cell corner: refine (top-left) or the
/// in-cart/owned quantity (bottom-right).
fn corner_text(text: String, color: Color, top_left: bool) -> impl Scene {
    let (left, right, top, bottom) = if top_left {
        (Val::Px(1.0), Val::Auto, Val::Px(0.0), Val::Auto)
    } else {
        (Val::Auto, Val::Px(1.0), Val::Auto, Val::Px(0.0))
    };
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(9.0)},
        }
        TextColor(color)
        Node {
            position_type: PositionType::Absolute,
            left: {left},
            right: {right},
            top: {top},
            bottom: {bottom},
        }
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

/// The cell's item name, wrapped tight to the cell's width rather than
/// truncated — the grid has no room for a tooltip layer.
fn cell_name(text: String) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(9.0)},
        }
        ThemeTextColor({TOKEN_TEXT})
        Node { width: px(CELL_SIZE + 10.0) }
        ignore_picking()
    }
}

fn price_label(text: String) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(10.0)},
        }
        ThemeTextColor({TOKEN_ACCENT})
        ignore_picking()
    }
}

fn side_column(detail: Option<DetailView>, cart: Vec<CartLineView>) -> impl Scene {
    bsn! {
        Node {
            width: px(170),
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
        }
        ignore_picking()
        Children [ detail_panel(detail), cart_panel(cart) ]
    }
}

fn detail_panel(detail: Option<DetailView>) -> impl Scene {
    let empty = detail.is_none();
    let filled = detail.map(|view| EntityScene(detail_content(view)));
    let empty_msg =
        empty.then(|| EntityScene(muted_text("Select an item to inspect it.".to_string())));
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            padding: {UiRect::all(px(10))},
            border: px(1),
            border_radius: BorderRadius::all(px(8)),
        }
        BackgroundColor(theme::FIELD)
        BorderColor::all(theme::GOLD_FAINT)
        ignore_picking()
        Children [ {filled}, {empty_msg} ]
    }
}

fn detail_content(view: DetailView) -> impl Scene {
    let icon = view.icon.map(|path| EntityScene(cell_icon(path)));
    let amount = view
        .amount
        .map(|amount| EntityScene(meta_row("You own".to_string(), amount.to_string())));
    let cards = (!view.cards.is_empty()).then(|| EntityScene(card_row(view.cards)));
    let description = view.description.map(|text| EntityScene(muted_text(text)));
    let price_label = if view.buy { "Unit price" } else { "Sell price" };

    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(6) }
        ignore_picking()
        Children [
            (
                Node {
                    width: px(40),
                    height: px(40),
                    position_type: PositionType::Relative,
                }
                ignore_picking()
                Children [ {icon} ]
            ),
            (
                Text({view.name})
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/cinzel.ttf"),
                    font_size: {FontSize::Px(13.0)},
                }
                ThemeTextColor({TOKEN_TEXT})
                ignore_picking()
            ),
            meta_row(price_label.to_string(), format!("{}z", view.price)),
            {amount},
            {cards},
            {description},
        ]
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

fn cart_panel(lines: Vec<CartLineView>) -> impl Scene {
    let empty = lines.is_empty();
    let rows: Vec<_> = lines.into_iter().map(cart_line).collect();
    let empty_msg = empty.then(|| EntityScene(muted_text("Cart is empty.".to_string())));
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
            padding: {UiRect::all(px(10))},
            border: px(1),
            border_radius: BorderRadius::all(px(8)),
        }
        BackgroundColor(theme::FIELD)
        BorderColor::all(theme::GOLD_FAINT)
        ignore_picking()
        Children [ {rows}, {empty_msg} ]
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
        }
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

fn zeny_display(zeny: u32) -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(6),
        }
        ignore_picking()
        Children [
            glyph_icon("coin", 13.0, theme::GOLD),
            (
                Text({format!("{zeny}z")})
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/cinzel.ttf"),
                    font_size: {FontSize::Px(13.0)},
                }
                ThemeTextColor({TOKEN_TEXT})
                ignore_picking()
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
    let sign = if buy { "-" } else { "+" };
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
            banner: None,
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
    fn cell_badge_prefers_cart_qty_over_owned() {
        assert_eq!(
            cell_badge(ShopTab::Sell, Some(3), Some(10)),
            Some("3".to_string())
        );
    }

    #[test]
    fn cell_badge_falls_back_to_owned_on_sell() {
        assert_eq!(
            cell_badge(ShopTab::Sell, None, Some(10)),
            Some("10".to_string())
        );
    }

    #[test]
    fn cell_badge_none_on_buy_without_cart() {
        assert_eq!(cell_badge(ShopTab::Buy, None, None), None);
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
