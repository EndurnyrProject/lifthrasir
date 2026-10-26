//! BSN scenes for the vending window: the chrome shell and the right panel's
//! editor, monitor and basket bodies. The left panel lives in [`super::source`].
//!
//! The chrome and the content host are spawned once; `rebuild_content` refills
//! the host from the current domain state. Typed fields therefore live only
//! inside that host and are rebuilt on membership changes, never per keystroke.

use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{EditableText, EditableTextFilter, FontSize, FontSourceTemplate};
use bevy::ui_widgets::TextInput;
use bevy_feathers::controls::FeathersButton;
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor, ThemeTextColor};
use lifthrasir_ui::theme;
use lifthrasir_ui::theme::feathers_theme::{
    TOKEN_PANEL_BG, TOKEN_TEXT, TOKEN_TEXT_DIM, TOKEN_TITLEBAR_BG, TOKEN_WINDOW_BG,
    TOKEN_WINDOW_BORDER,
};
use lifthrasir_ui::widgets::chrome::{chrome_text, glyph_icon, ignore_picking};

use crate::widgets::vending_window::cells::{ItemCellView, item_cell};
use crate::widgets::vending_window::drag::{
    DragKind, DragSource, DropZone, on_drag_start, on_zone_drop,
};
use crate::widgets::vending_window::source::VendingSourceHost;
use crate::widgets::vending_window::{
    BasketRemove, BasketStep, DraftFieldKind, VendingBasketRow, VendingCancelButton,
    VendingContentHost, VendingDraftField, VendingErrorLine, VendingModeNote, VendingOpenButton,
    VendingRowRemove, VendingTitle, VendingTitleField, VendingWindowBody, VendingWindowRoot,
    VendingWindowTitlebar, on_basket_refresh, on_basket_remove, on_basket_step, on_cancel_setup,
    on_checkout, on_close_button, on_close_shop, on_item_inspect, on_open_shop, on_row_removed,
};

const WINDOW_LEFT: f32 = 250.0;
const WINDOW_TOP: f32 = 80.0;
/// Width of the right panel; the window grows by the left panel when it shows.
const MAIN_WIDTH: f32 = 420.0;
/// Minimum height of the setup editor's drop zone, so an empty shop is an
/// obvious target.
const LISTINGS_MIN_HEIGHT: f32 = 150.0;

/// One listing being drafted, with its current typed values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftRowView {
    pub cart_index: u32,
    pub cell: ItemCellView,
    pub label: String,
    pub stock: u32,
    pub amount_input: String,
    pub price_input: String,
}

/// One accepted listing shown in the read-only monitor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShopRowView {
    pub index: u32,
    pub cell: ItemCellView,
    pub label: String,
    pub amount: u32,
    pub price: u32,
}

/// One staged basket line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasketRowView {
    pub index: u32,
    pub cell: ItemCellView,
    pub label: String,
    pub price: u32,
    pub quantity: u32,
    pub line_total: u64,
}

/// One reported sale shown under the monitor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaleRowView {
    pub cell: ItemCellView,
    pub label: String,
    pub amount: u32,
    pub zeny: u32,
    pub buyer: String,
}

/// The whole window: titlebar plus the body region the rebuild system fills.
pub fn window() -> impl Scene {
    bsn! {
        VendingWindowRoot
        Node {
            position_type: PositionType::Absolute,
            left: px(WINDOW_LEFT),
            top: px(WINDOW_TOP),
            flex_direction: FlexDirection::Column,
            border: px(1),
            border_radius: BorderRadius::all(px(12)),
        }
        ThemeBackgroundColor({TOKEN_WINDOW_BG})
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        Visibility::Hidden
        Children [
            @titlebar() --
            @body()
        ]
    }
}

fn titlebar() -> impl Scene {
    let title = "Vending".to_string();
    bsn! {
        VendingWindowTitlebar
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(8),
            padding: {UiRect::axes(px(14), px(11))},
            border: {UiRect { bottom: Val::Px(1.0), ..default() }},
        }
        ThemeBackgroundColor({TOKEN_TITLEBAR_BG})
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        Pickable
        on(lifthrasir_ui::widgets::chrome::drag_window::<VendingWindowTitlebar, VendingWindowRoot>)
        Children [
            @glyph_icon("coin", 16.0, theme::GOLD) --
            VendingTitle
            Text(title)
            TextFont {
                font: FontSourceTemplate::Handle("ro://fonts/cinzel.ttf"),
                font_size: {FontSize::Px(15.0)},
            }
            ThemeTextColor({TOKEN_TEXT})
            Node { flex_grow: 1.0 }
            @ignore_picking() --
            @FeathersButton { @caption: bsn! { @glyph_icon("close", 13.0, theme::TEXT_DIM) } }
            Node { width: px(22), height: px(22) }
            on(on_close_button)
        ]
    }
}

fn body() -> impl Scene {
    bsn! {
        VendingWindowBody
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::FlexStart,
            padding: {UiRect::all(px(14))},
            column_gap: px(12),
        }
        @ignore_picking()
        Children [
            VendingSourceHost
            Node { display: Display::None, flex_direction: FlexDirection::Column }
            @ignore_picking() --
            VendingContentHost
            Node { width: px(MAIN_WIDTH), flex_direction: FlexDirection::Column, row_gap: px(10) }
            @ignore_picking()
        ]
    }
}

/// The setup editor: the title, the drafted listings (a drop zone for cart
/// stock), the slot count, and submission.
pub fn editor(
    rows: Vec<DraftRowView>,
    title: String,
    note: String,
    slots: String,
    error: Option<String>,
) -> impl Scene {
    let listings: Vec<_> = rows.into_iter().map(draft_row).collect();
    let empty = listings
        .is_empty()
        .then(|| EntityScene(drop_hint("Drag items here from your cart.".to_string())));
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(10) }
        @ignore_picking()
        Children [
            @section_label("Shop title".to_string()) --
            @title_field(title) --
            DropZone::SetupListings
            Node {
                min_height: px(LISTINGS_MIN_HEIGHT),
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                padding: {UiRect::all(px(8))},
                border: px(1),
                border_radius: BorderRadius::all(px(8)),
            }
            ThemeBackgroundColor({TOKEN_PANEL_BG})
            BorderColor::all(theme::GOLD_FAINT)
            Pickable
            on(on_zone_drop)
            Children [ {listings} -- {empty} ] --
            Node { flex_direction: FlexDirection::Row, justify_content: JustifyContent::SpaceBetween }
            @ignore_picking()
            Children [
                @chrome_text(slots, 11.0, theme::TEXT_DIM) --
                @mode_note(note)
            ] --
            @error_line(error) --
            Node { flex_direction: FlexDirection::Row, column_gap: px(8) }
            @ignore_picking()
            Children [
                VendingCancelButton
                @FeathersButton { @caption: bsn! { @chrome_text("Cancel".to_string(), 12.0, theme::TEXT_DIM) } }
                Node { height: px(34) }
                on(on_cancel_setup) --
                Node { flex_grow: 1.0 }
                @ignore_picking() --
                VendingOpenButton
                @FeathersButton { @caption: bsn! { @chrome_text("Open Shop".to_string(), 12.0, theme::TEXT) } }
                Node { width: px(120), height: px(34) }
                BackgroundColor({theme::EMERALD})
                on(on_open_shop)
            ]
        ]
    }
}

/// A centred faint line telling the player where to drop.
fn drop_hint(text: String) -> impl Scene {
    bsn! {
        Node {
            flex_grow: 1.0,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            padding: {UiRect::all(px(16))},
        }
        @ignore_picking()
        Children [ @chrome_text(text, 12.0, theme::TEXT_FAINT) ]
    }
}

/// The read-only monitor: remaining stock, net earnings, and recent sales.
pub fn monitor(
    rows: Vec<ShopRowView>,
    sales: Vec<SaleRowView>,
    note: String,
    error: Option<String>,
) -> impl Scene {
    let listings: Vec<_> = rows.into_iter().map(shop_row).collect();
    let sales: Vec<_> = sales.into_iter().map(sale_row).collect();
    let empty = listings
        .is_empty()
        .then(|| EntityScene(muted("Everything sold.".to_string())));
    let no_sales = sales
        .is_empty()
        .then(|| EntityScene(muted("No sales yet.".to_string())));
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(10) }
        @ignore_picking()
        Children [
            @mode_note(note) --
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                padding: {UiRect::all(px(8))},
                border: px(1),
                border_radius: BorderRadius::all(px(8)),
            }
            ThemeBackgroundColor({TOKEN_PANEL_BG})
            @ignore_picking()
            Children [
                @section_label("Remaining stock".to_string()) --
                Node { flex_direction: FlexDirection::Column, row_gap: px(6) }
                @ignore_picking()
                Children [ {listings} -- {empty} ]
            ] --
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                padding: {UiRect::all(px(8))},
                border: px(1),
                border_radius: BorderRadius::all(px(8)),
            }
            ThemeBackgroundColor({TOKEN_PANEL_BG})
            @ignore_picking()
            Children [
                @section_label("Recent sales".to_string()) --
                Node { flex_direction: FlexDirection::Column, row_gap: px(6) }
                @ignore_picking()
                Children [ {sales} -- {no_sales} ]
            ] --
            @error_line(error) --
            Node { flex_direction: FlexDirection::Row, column_gap: px(8) }
            @ignore_picking()
            Children [
                @FeathersButton { @caption: bsn! { @chrome_text("Close Shop".to_string(), 12.0, theme::TEXT) } }
                Node { width: px(120), height: px(34) }
                BackgroundColor({theme::EMERALD})
                on(on_close_shop)
            ]
        ]
    }
}

fn mode_note(note: String) -> impl Scene {
    bsn! {
        VendingModeNote
        Text(note)
        TextFont {
            font: FontSourceTemplate::Handle(lifthrasir_ui::theme::FONT_BODY),
            font_size: {FontSize::Px(12.0)},
        }
        ThemeTextColor({TOKEN_TEXT_DIM})
        @ignore_picking()
    }
}

fn section_label(text: String) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("ro://fonts/cinzel.ttf"),
            font_size: {FontSize::Px(12.0)},
        }
        ThemeTextColor({TOKEN_TEXT})
        @ignore_picking()
    }
}

fn muted(text: String) -> impl Scene {
    chrome_text(text, 12.0, theme::TEXT_DIM)
}

fn title_field(title: String) -> impl Scene {
    bsn! {
        VendingTitleField
        EditableText::new(title)
        TextInput
        EditableTextFilter::new(|c| !c.is_control())
        TextFont {
            font: FontSourceTemplate::Handle(lifthrasir_ui::theme::FONT_BODY),
            font_size: {FontSize::Px(13.0)},
        }
        TextColor({theme::TEXT})
        Node {
            height: px(32),
            padding: {UiRect::axes(px(10), px(6))},
            border: px(1),
            border_radius: BorderRadius::all(px(7)),
        }
        BackgroundColor({theme::FIELD})
        BorderColor::all(theme::STROKE)
    }
}

/// One drafted listing: its icon (the drag handle back to the stock panel),
/// name, quantity and price fields, and a remove button.
fn draft_row(row: DraftRowView) -> impl Scene {
    let cart_index = row.cart_index;
    let stock = format!("{} in cart", row.stock);
    bsn! {
        Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(6) }
        @ignore_picking()
        Children [
            @item_cell(row.cell)
            DragSource { kind: DragKind::Draft, index: cart_index }
            Pickable
            on(on_drag_start)
            on(on_item_inspect) --
            Node { flex_grow: 1.0, min_width: px(0), flex_direction: FlexDirection::Column, overflow: {Overflow::clip()} }
            @ignore_picking()
            Children [
                @chrome_text(row.label, 12.0, theme::TEXT) --
                @chrome_text(stock, 10.0, theme::TEXT_FAINT)
            ] --
            @draft_field(cart_index, DraftFieldKind::Amount, row.amount_input, 48.0) --
            @chrome_text("x".to_string(), 11.0, theme::TEXT_FAINT) --
            @draft_field(cart_index, DraftFieldKind::Price, row.price_input, 84.0) --
            @chrome_text("z".to_string(), 11.0, theme::GOLD) --
            VendingRowRemove { cart_index: cart_index }
            @FeathersButton { @caption: bsn! { @glyph_icon("close", 11.0, theme::TEXT_DIM) } }
            Node { width: px(24), height: px(24) }
            on(on_row_removed)
        ]
    }
}

fn draft_field(cart_index: u32, kind: DraftFieldKind, value: String, width: f32) -> impl Scene {
    bsn! {
        VendingDraftField { cart_index, kind }
        EditableText::new(value)
        TextInput
        EditableTextFilter::new(|c| c.is_ascii_digit())
        TextFont {
            font: FontSourceTemplate::Handle(lifthrasir_ui::theme::FONT_BODY),
            font_size: {FontSize::Px(12.0)},
        }
        TextColor({theme::TEXT})
        Node {
            width: px(width),
            height: px(26),
            padding: {UiRect::axes(px(8), px(4))},
            border: px(1),
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor({theme::FIELD})
        BorderColor::all(theme::STROKE)
    }
}

/// One listing of the seller's open shop; its icon opens the item details.
fn shop_row(row: ShopRowView) -> impl Scene {
    let index = row.index;
    let detail = format!("x{}  -  {}z each", row.amount, row.price);
    bsn! {
        Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(8) }
        @ignore_picking()
        Children [
            @item_cell(row.cell)
            DragSource { kind: DragKind::Shop, index: index }
            Pickable
            on(on_item_inspect) --
            Node { flex_grow: 1.0, min_width: px(0), overflow: {Overflow::clip()} }
            @ignore_picking()
            Children [ @chrome_text(row.label, 12.0, theme::TEXT) ] --
            @chrome_text(detail, 12.0, theme::TEXT_DIM)
        ]
    }
}

fn sale_row(sale: SaleRowView) -> impl Scene {
    let detail = format!("x{}  -  {}z  -  {}", sale.amount, sale.zeny, sale.buyer);
    icon_row(sale.cell, sale.label, detail)
}

/// A read-only row: item icon, name, and a dim detail on the right.
fn icon_row(cell: ItemCellView, label: String, detail: String) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(8) }
        @ignore_picking()
        Children [
            @item_cell(cell)
            @ignore_picking() --
            Node { flex_grow: 1.0, min_width: px(0), overflow: {Overflow::clip()} }
            @ignore_picking()
            Children [ @chrome_text(label, 12.0, theme::TEXT) ] --
            @chrome_text(detail, 12.0, theme::TEXT_DIM)
        ]
    }
}

fn error_line(error: Option<String>) -> impl Scene {
    let text = error.unwrap_or_default();
    bsn! {
        VendingErrorLine
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle(lifthrasir_ui::theme::FONT_BODY),
            font_size: {FontSize::Px(12.0)},
        }
        TextColor({theme::GOLD})
        @ignore_picking()
    }
}

/// The buyer's basket: staged lines (a drop zone for the vendor's listings),
/// the total, and checkout.
pub fn buyer(
    rows: Vec<BasketRowView>,
    note: String,
    guidance: Option<String>,
    error: Option<String>,
) -> impl Scene {
    let rows: Vec<_> = rows.into_iter().map(basket_row).collect();
    let guidance = guidance.map(|text| EntityScene(muted(text)));
    let empty = rows
        .is_empty()
        .then(|| EntityScene(drop_hint("Drag items here from the shop.".to_string())));
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(10) }
        @ignore_picking()
        Children [
            @section_label("Your purchase".to_string()) --
            DropZone::Basket
            Node {
                min_height: px(LISTINGS_MIN_HEIGHT),
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                padding: {UiRect::all(px(8))},
                border: px(1),
                border_radius: BorderRadius::all(px(8)),
            }
            ThemeBackgroundColor({TOKEN_PANEL_BG})
            BorderColor::all(theme::GOLD_FAINT)
            Pickable
            on(on_zone_drop)
            Children [ {rows} -- {empty} ] --
            Node { flex_direction: FlexDirection::Row, justify_content: JustifyContent::FlexEnd }
            @ignore_picking()
            Children [ @plain_note(note) ] --
            {guidance} --
            @error_line(error) --
            Node { flex_direction: FlexDirection::Row, column_gap: px(8) }
            @ignore_picking()
            Children [
                @FeathersButton { @caption: bsn! { @chrome_text("Refresh".to_string(), 12.0, theme::TEXT_DIM) } }
                Node { height: px(34) }
                on(on_basket_refresh) --
                Node { flex_grow: 1.0 }
                @ignore_picking() --
                @FeathersButton { @caption: bsn! { @chrome_text("Buy".to_string(), 12.0, theme::TEXT) } }
                Node { width: px(120), height: px(34) }
                BackgroundColor({theme::EMERALD})
                on(on_checkout)
            ]
        ]
    }
}

/// One staged line: icon (drag it back to the shop to remove), name and unit
/// price, a quantity stepper, the line total, and a remove button.
fn basket_row(view: BasketRowView) -> impl Scene {
    let index = view.index;
    let unit = format!("{}z each", view.price);
    let quantity = view.quantity.to_string();
    let total = format!("{}z", view.line_total);
    bsn! {
        VendingBasketRow { index }
        Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(6) }
        @ignore_picking()
        Children [
            @item_cell(view.cell)
            DragSource { kind: DragKind::Basket, index: index }
            Pickable
            on(on_drag_start)
            on(on_item_inspect) --
            Node { flex_grow: 1.0, min_width: px(0), flex_direction: FlexDirection::Column, overflow: {Overflow::clip()} }
            @ignore_picking()
            Children [
                @chrome_text(view.label, 12.0, theme::TEXT) --
                @chrome_text(unit, 10.0, theme::TEXT_FAINT)
            ] --
            BasketStep { index, up: false }
            @FeathersButton { @caption: bsn! { @glyph_icon("minus", 10.0, theme::TEXT) } }
            Node { width: px(22), height: px(22) }
            on(on_basket_step) --
            Node { min_width: px(24), justify_content: JustifyContent::Center }
            @ignore_picking()
            Children [ @chrome_text(quantity, 12.0, theme::TEXT) ] --
            BasketStep { index, up: true }
            @FeathersButton { @caption: bsn! { @glyph_icon("plus", 10.0, theme::TEXT) } }
            Node { width: px(22), height: px(22) }
            on(on_basket_step) --
            Node { min_width: px(64), justify_content: JustifyContent::FlexEnd }
            @ignore_picking()
            Children [ @chrome_text(total, 12.0, theme::GOLD) ] --
            BasketRemove { index }
            @FeathersButton { @caption: bsn! { @glyph_icon("close", 11.0, theme::TEXT_DIM) } }
            Node { width: px(24), height: px(24) }
            on(on_basket_remove)
        ]
    }
}

fn plain_note(note: String) -> impl Scene {
    bsn! {
        Text(note)
        TextFont {
            font: FontSourceTemplate::Handle(lifthrasir_ui::theme::FONT_BODY),
            font_size: {FontSize::Px(12.0)},
        }
        ThemeTextColor({TOKEN_TEXT})
        @ignore_picking()
    }
}
