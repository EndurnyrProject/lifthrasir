//! BSN scenes for the vending window: one chrome shell and its two bodies.
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

use crate::widgets::vending_window::{
    BasketStep, DraftFieldKind, VendingBasketRow, VendingCancelButton, VendingContentHost,
    VendingDraftField, VendingErrorLine, VendingModeNote, VendingOpenButton, VendingRowRemove,
    VendingStockCell, VendingTitle, VendingTitleField, VendingWindowBody, VendingWindowRoot,
    VendingWindowTitlebar, on_basket_refresh, on_basket_step, on_cancel_setup, on_checkout,
    on_close_button, on_close_shop, on_listing_click, on_open_shop, on_row_removed,
    on_stock_toggled,
};

const WINDOW_LEFT: f32 = 250.0;
const WINDOW_TOP: f32 = 80.0;
const WINDOW_WIDTH: f32 = 468.0;

/// One sellable cart slot offered as a stock cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockCellView {
    pub cart_index: u32,
    pub label: String,
    pub amount: u32,
    pub selected: bool,
}

/// One listing being drafted, with its current typed values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftRowView {
    pub cart_index: u32,
    pub label: String,
    pub stock: u32,
    pub amount_input: String,
    pub price_input: String,
}

/// One accepted listing shown in the read-only monitor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShopRowView {
    pub label: String,
    pub amount: u32,
    pub price: u32,
}

/// One listing row of the buyer body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowseRowView {
    pub index: u32,
    pub label: String,
    pub stock: u32,
    pub price: u32,
    pub quantity: u32,
}

/// One reported sale shown under the monitor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaleRowView {
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
            width: px(WINDOW_WIDTH),
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
            flex_direction: FlexDirection::Column,
            padding: {UiRect::all(px(14))},
            row_gap: px(10),
        }
        @ignore_picking()
        Children [
            VendingContentHost
            Node { flex_direction: FlexDirection::Column, row_gap: px(10) }
            @ignore_picking()
        ]
    }
}

/// The setup editor: sellable stock, the title, and the drafted listings.
pub fn editor(
    stock: Vec<StockCellView>,
    rows: Vec<DraftRowView>,
    title: String,
    note: String,
    error: Option<String>,
) -> impl Scene {
    let cells: Vec<_> = stock.into_iter().map(stock_cell).collect();
    let listings: Vec<_> = rows.into_iter().map(draft_row).collect();
    let empty = listings
        .is_empty()
        .then(|| EntityScene(muted("No items listed yet.".to_string())));
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(10) }
        @ignore_picking()
        Children [
            @mode_note(note) --
            Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: px(6),
                row_gap: px(6),
            }
            @ignore_picking()
            Children [ {cells} ] --
            @section_label("Shop title".to_string()) --
            @title_field(title) --
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
                @section_label("Listings".to_string()) --
                Node { flex_direction: FlexDirection::Column, row_gap: px(6) }
                @ignore_picking()
                Children [ {listings} -- {empty} ]
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

fn stock_cell(cell: StockCellView) -> impl Scene {
    let label = format!("{} x{}", cell.label, cell.amount);
    let cart_index = cell.cart_index;
    let background = if cell.selected {
        theme::EMERALD
    } else {
        theme::GLASS_2
    };
    bsn! {
        VendingStockCell { cart_index: cart_index }
        @FeathersButton { @caption: bsn! { @chrome_text(label, 12.0, theme::TEXT) } }
        Node { height: px(28), padding: {UiRect::axes(px(8), px(4))} }
        BackgroundColor(background)
        on(on_stock_toggled)
    }
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

fn draft_row(row: DraftRowView) -> impl Scene {
    let label = format!("{} (x{})", row.label, row.stock);
    let cart_index = row.cart_index;
    bsn! {
        Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(6) }
        @ignore_picking()
        Children [
            Node { flex_grow: 1.0, min_width: px(0) }
            @ignore_picking()
            Children [ @chrome_text(label, 12.0, theme::TEXT) ] --
            @draft_field(cart_index, DraftFieldKind::Amount, row.amount_input, 52.0) --
            @draft_field(cart_index, DraftFieldKind::Price, row.price_input, 84.0) --
            @FeathersButton { @caption: bsn! { @chrome_text("Remove".to_string(), 11.0, theme::TEXT_DIM) } }
            Node { height: px(26) }
            VendingRowRemove { cart_index: cart_index }
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

fn shop_row(row: ShopRowView) -> impl Scene {
    let detail = format!("x{}  -  {}z each", row.amount, row.price);
    row_line(row.label, detail, theme::TEXT)
}

fn sale_row(sale: SaleRowView) -> impl Scene {
    let detail = format!("x{}  -  {}z  -  {}", sale.amount, sale.zeny, sale.buyer);
    row_line(sale.label, detail, theme::TEXT)
}

fn row_line(label: String, detail: String, color: Color) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(6) }
        @ignore_picking()
        Children [
            Node { flex_grow: 1.0, min_width: px(0) }
            @ignore_picking()
            Children [ @chrome_text(label, 12.0, color) ] --
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

/// The buyer body: one row per listing, the staged total, and checkout.
pub fn buyer(
    rows: Vec<BrowseRowView>,
    note: String,
    guidance: Option<String>,
    error: Option<String>,
) -> impl Scene {
    let rows: Vec<_> = rows.into_iter().map(browse_row).collect();
    let guidance = guidance.map(|text| EntityScene(muted(text)));
    let empty = rows
        .is_empty()
        .then(|| EntityScene(muted("This shop has nothing left.".to_string())));
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(10) }
        @ignore_picking()
        Children [
            @plain_note(note) --
            {guidance} --
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
                @section_label("For sale".to_string()) --
                Node { flex_direction: FlexDirection::Column, row_gap: px(6) }
                @ignore_picking()
                Children [ {rows} -- {empty} ]
            ] --
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

fn browse_row(view: BrowseRowView) -> impl Scene {
    let index = view.index;
    let detail = format!("{} / x{} at {}z", view.label, view.stock, view.price);
    let staged = if view.quantity == 0 {
        "Not in basket".to_string()
    } else {
        format!("In basket: x{}", view.quantity)
    };
    bsn! {
        VendingBasketRow { index }
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(6),
            padding: {UiRect::axes(px(6), px(4))},
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor({theme::GLASS_2})
        Pickable::default()
        on(on_listing_click)
        Children [
            Node { flex_grow: 1.0, min_width: px(0), flex_direction: FlexDirection::Column, row_gap: px(2) }
            @ignore_picking()
            Children [
                @chrome_text(detail, 12.0, theme::TEXT) --
                @chrome_text(staged, 11.0, theme::TEXT_DIM)
            ] --
            BasketStep { index, up: false }
            @FeathersButton { @caption: bsn! { @chrome_text("-".to_string(), 12.0, theme::TEXT) } }
            Node { width: px(26), height: px(26) }
            on(on_basket_step) --
            BasketStep { index, up: true }
            @FeathersButton { @caption: bsn! { @chrome_text("+".to_string(), 12.0, theme::TEXT) } }
            Node { width: px(26), height: px(26) }
            on(on_basket_step)
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
