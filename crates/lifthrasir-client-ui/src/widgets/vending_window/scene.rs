//! The vending window scenes: the frame [`window`] spawns once and the four bodies
//! `rebuild_content` projects from their view-models. Sizes follow the `Shop Setup`,
//! `Seller View` and `Buyer View` artboards of the Lifthrasir Widgets design.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{EditableText, EditableTextFilter, FontSize, FontSourceTemplate};
use bevy::ui_widgets::{ControlOrientation, ScrollArea, TextInput};
use bevy_feathers::controls::FeathersScrollbar;
use lifthrasir_ui::theme::{self, ro_dark};
use lifthrasir_ui::widgets::chrome::{chrome_text, drag_window, glyph_icon, ignore_picking};
use lifthrasir_ui::widgets::primitives::{
    BUTTON_SHADOW, ButtonArt, HoverColors, button_with, close_button, diamond, display_text,
    ghost_button, inset, window_title_bar, window_titled,
};

use super::view::{
    BasketLineView, BuyRowView, BuyerView, CONFIRM_FROM, ClosedView, ListingRowView, SaleView,
    SellerView, SetupView, ShopStockView, StockRowView, StockState, counted, words, zeny,
};
use super::{
    DraftFieldKind, SetupReadout, Surface, VendingAction, VendingContentHost, VendingDraftField,
    VendingItem, VendingOpenButton, VendingQtyField, VendingScroll, VendingTitleField,
    VendingWindowRoot, VendingWindowTitlebar, on_item_inspect, on_vending_button,
};
use crate::screens::shared::{
    DANGER_TINT, QTY_DIGITS, Ring, chip, colored_lines, number_field, overline, row_button,
    shop_sign, sign_title, slot_frame, small_button, thousands, usable, wide_button,
};
use crate::widgets::inventory_window::scene::{CellView, slot_marks};

const WINDOW_LEFT: f32 = 140.0;
const WINDOW_TOP: f32 = 4.0;
/// Height of the setup body's two lists.
const SETUP_LIST: f32 = 430.0;
/// Height of the seller's stock and sales lists.
const SHOP_LIST: f32 = 392.0;
/// Height of the buyer's two columns: the basket, and the listings under their caption.
const BUY_COLUMN: f32 = 454.0;
const SLOT: f32 = 34.0;
const ICON: f32 = 26.0;
const BUY_SLOT: f32 = 38.0;
const BUY_ICON: f32 = 28.0;
const LINE_SLOT: f32 = 30.0;
const LINE_ICON: f32 = 22.0;
/// A price never passes 1,000,000,000 z.
const PRICE_DIGITS: usize = 10;

/// The frame: the title bar and the empty body `rebuild_content` fills. The window
/// takes its width from the body it shows.
pub fn window() -> impl Scene {
    bsn! {
        VendingWindowRoot
        @window_titled()
        Node {
            position_type: PositionType::Absolute,
            left: px(WINDOW_LEFT),
            top: px(WINDOW_TOP),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
        }
        Visibility::Hidden
        Pickable
        Children [
            VendingWindowTitlebar
            @window_title_bar("Vending", bsn! {
                @close_button()
                VendingAction::CloseWindow
                on(on_vending_button)
            })
            Pickable
            on(drag_window::<VendingWindowTitlebar, VendingWindowRoot>)
            --
            VendingContentHost
            Node {
                flex_direction: FlexDirection::Column,
                margin: {UiRect::new(px(0), px(0), px(10), px(8))},
            }
            @ignore_picking()
        ]
    }
}

/// A list row `height` px high with a rule under it. A right-click inspects `item`.
fn item_row(item: VendingItem, height: f32) -> impl Scene {
    let colors = HoverColors {
        rest: (Color::NONE, ro_dark::INSET_RULE),
        hover: (Color::WHITE.with_alpha(0.02), ro_dark::INSET_RULE),
    };
    bsn! {
        item
        Hovered
        colors
        Node {
            height: px(height),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            column_gap: px(10),
            padding: {UiRect::horizontal(px(10))},
            border: {UiRect::bottom(px(1))},
        }
        BackgroundColor(Color::NONE)
        BorderColor::all(ro_dark::INSET_RULE)
        Pickable
        on(on_item_inspect)
    }
}

/// The item's slot with its refine and card marks.
fn slot(cell: CellView, size: f32, icon: f32) -> impl Scene {
    let ring = cell.ring;
    let marks = slot_marks(&cell, icon, true);
    bsn! {
        @slot_frame(size, ring)
        @ignore_picking()
        Children [ {marks} ]
    }
}

fn name_color(cell: &CellView) -> Color {
    if cell.ring == Ring::Rare {
        ro_dark::RARE_RING
    } else {
        ro_dark::STRONG
    }
}

fn name_line(name: String, size: f32, color: Color) -> impl Scene {
    bsn! {
        @chrome_text(name, size, color)
        TextLayout { linebreak: LineBreak::NoWrap }
    }
}

fn sub_line(text: String) -> impl Scene {
    bsn! {
        @chrome_text(text, 11.0, ro_dark::MUTED)
        TextLayout { linebreak: LineBreak::NoWrap }
    }
}

/// Two lines taking the row's spare width and clipped to it.
fn stacked(top: impl Scene, bottom: impl Scene) -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            flex_basis: px(0),
            min_width: px(0),
            overflow: {Overflow::clip()},
        }
        @ignore_picking()
        Children [ @top -- @bottom ]
    }
}

/// A right-aligned figure in a column `width` px wide.
fn figure(text: String, width: f32, color: Color) -> impl Scene {
    bsn! {
        Node { width: px(width), flex_shrink: 0.0, justify_content: JustifyContent::FlexEnd }
        @ignore_picking()
        Children [ @chrome_text(text, 12.0, color) ]
    }
}

/// The caption over a panel: its name, then a count.
fn panel_head(title: &'static str, count: String, count_color: Color) -> impl Scene {
    bsn! {
        Node {
            height: px(22),
            margin: {UiRect::bottom(px(6))},
            align_items: AlignItems::Center,
            column_gap: px(8),
        }
        @ignore_picking()
        Children [
            @overline(title.into(), ro_dark::ACCENT)
            --
            @chrome_text(count, 12.0, count_color)
        ]
    }
}

/// The column captions over a list whose rows start with a `lead` px slot.
fn list_head<S: Scene>(lead: f32, columns: Vec<S>) -> impl Scene {
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
            Node { width: px(lead), flex_shrink: 0.0 }
            @ignore_picking()
            --
            @overline("ITEM".into(), ro_dark::ACCENT)
            Node { flex_grow: 1.0 }
            --
            {columns}
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

/// A scrolling column of `rows` filling the rest of its panel, with the scrollbar
/// pinned to its right. `message` is centred over an empty list.
fn scroll_rows<S: Scene>(
    kind: VendingScroll,
    scroll: Vec2,
    rows: Vec<S>,
    message: Option<&'static str>,
) -> impl Scene {
    let message = message.map(|text| EntityScene(list_message(text)));
    bsn! {
        Node { flex_grow: 1.0, min_height: px(0), position_type: PositionType::Relative }
        @ignore_picking()
        Children [
            #list
            kind
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
    }
}

fn list_message(text: &'static str) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(16),
            right: px(16),
            top: px(80),
            justify_content: JustifyContent::Center,
        }
        @ignore_picking()
        Children [
            @chrome_text(text.into(), 12.0, ro_dark::MUTED)
            TextLayout { justify: Justify::Center }
        ]
    }
}

/// A caption over a value, for the read-outs along a footer.
fn stat(label: &'static str, value: impl Scene) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Column, flex_shrink: 0.0 }
        @ignore_picking()
        Children [ @chrome_text(label.into(), 11.0, ro_dark::MUTED) -- @value ]
    }
}

fn step_button(icon: &'static str, action: VendingAction) -> impl Scene {
    bsn! {
        @small_button(icon, 24.0)
        action
        on(on_vending_button)
    }
}

/// The setup body: the shop name beside its sign preview, the cart beside the drafted
/// listings, then the totals and Open Shop. `scroll` is the cart's and the listings'.
pub(super) fn setup(view: SetupView, scroll: [Vec2; 2]) -> impl Scene {
    let [stock_scroll, listing_scroll] = scroll;
    let full = view.listings.len() >= view.max_slots as usize;
    let slots = format!("{} / {}", view.listings.len(), view.max_slots);
    let slots_color = if full {
        ro_dark::STATUS_BUSY
    } else {
        ro_dark::STRONG
    };
    let stock_count = counted(view.stock.len(), "item");
    let no_stock = view.stock.is_empty().then_some("Your cart is empty");
    let no_listings = view
        .listings
        .is_empty()
        .then_some("List items from your cart to sell them");
    let stock: Vec<_> = view.stock.into_iter().map(stock_row).collect();
    let listings: Vec<_> = view.listings.into_iter().map(listing_row).collect();
    let heads = vec![
        column_head("QTY", 64.0),
        column_head("PRICE EACH", 128.0),
        column_head("IF ALL SELL", 96.0),
        column_head("", 22.0),
    ];
    let cost = format!("{} SP \u{b7} Already paid", view.sp_spent);
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(16) }
        @ignore_picking()
        Children [
            @name_row(view.title, view.title_limit)
            --
            Node { column_gap: px(20), align_items: AlignItems::FlexStart }
            @ignore_picking()
            Children [
                Node { width: px(330), flex_shrink: 0.0, flex_direction: FlexDirection::Column }
                @ignore_picking()
                Children [
                    @panel_head("IN YOUR CART", stock_count, ro_dark::STRONG)
                    --
                    @inset()
                    Node { height: px(SETUP_LIST), flex_direction: FlexDirection::Column }
                    @ignore_picking()
                    Children [ @scroll_rows(VendingScroll::Stock, stock_scroll, stock, no_stock) ]
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
                    @panel_head("LISTINGS", slots, slots_color)
                    --
                    @inset()
                    Node { height: px(SETUP_LIST), flex_direction: FlexDirection::Column }
                    @ignore_picking()
                    Children [
                        @list_head(SLOT, heads)
                        --
                        @scroll_rows(VendingScroll::Listings, listing_scroll, listings, no_listings)
                    ]
                ]
            ]
            --
            @setup_footer(cost)
        ]
    }
}

/// The shop name field with its length count, beside the sign as others will see it.
fn name_row(title: String, limit: u32) -> impl Scene {
    let field = EditableText {
        max_characters: Some(limit as usize),
        ..EditableText::new(title)
    };
    bsn! {
        Node { column_gap: px(20) }
        @ignore_picking()
        Children [
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                flex_basis: px(0),
                min_width: px(0),
                row_gap: px(6),
            }
            @ignore_picking()
            Children [
                Node {
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                }
                @ignore_picking()
                Children [
                    @overline("SHOP NAME".into(), ro_dark::ACCENT)
                    --
                    SetupReadout::TitleCount
                    @chrome_text(String::new(), 11.0, ro_dark::MUTED)
                ]
                --
                VendingTitleField
                field
                TextInput
                EditableTextFilter::new(|c| !c.is_control())
                TextFont {
                    font: FontSourceTemplate::Handle(theme::FONT_BODY),
                    font_size: {FontSize::Px(13.0)},
                }
                TextColor(ro_dark::BODY)
                Node {
                    height: px(32),
                    padding: {UiRect::axes(px(10), px(6))},
                    border: px(1),
                    border_radius: BorderRadius::all(px(5)),
                }
                BackgroundColor(ro_dark::INSET)
                BorderColor::all(ro_dark::SLOT_STROKE)
                --
                @sub_line("Shown above your head while the shop is open".into())
            ]
            --
            Node {
                width: px(300),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
            }
            @ignore_picking()
            Children [
                @overline("PREVIEW".into(), ro_dark::ACCENT)
                --
                @sign_preview()
            ]
        ]
    }
}

/// The sign over a small figure, as it will hang in the world.
fn sign_preview() -> impl Scene {
    bsn! {
        Node {
            height: px(74),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: {UiRect::top(px(10))},
            border: px(1),
            border_radius: BorderRadius::all(px(6)),
            position_type: PositionType::Relative,
        }
        BackgroundColor(ro_dark::BAR_TROUGH)
        BorderColor::all(ro_dark::SLOT_STROKE)
        @ignore_picking()
        Children [
            @shop_sign(
                bsn! { SetupReadout::Sign @sign_title(String::new()) },
                260.0,
                Color::NONE,
            )
            @ignore_picking()
            --
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                bottom: px(3),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
            }
            @ignore_picking()
            Children [
                Node { width: px(14), height: px(14), border_radius: BorderRadius::MAX }
                BackgroundColor(ro_dark::TAB_ACTIVE)
                @ignore_picking()
                --
                Node {
                    width: px(20),
                    height: px(15),
                    margin: {UiRect::top(px(-1))},
                    border_radius: {BorderRadius::top(px(10))},
                }
                BackgroundColor(ro_dark::TAB_ACTIVE)
                @ignore_picking()
            ]
        ]
    }
}

/// One cart slot: its item, how many it holds, and whether it can be listed.
fn stock_row(view: StockRowView) -> impl Scene {
    let item = VendingItem {
        surface: Surface::Cart,
        index: view.cart_index,
    };
    let color = if view.state == StockState::Listed {
        ro_dark::MUTED
    } else {
        name_color(&view.cell)
    };
    let control = stock_control(view.cart_index, view.state);
    bsn! {
        @item_row(item, 46.0)
        Children [
            @slot(view.cell, SLOT, ICON)
            --
            @stacked(name_line(view.name, 12.0, color), sub_line(view.sub))
            --
            {control}
        ]
    }
}

fn stock_control(cart_index: u32, state: StockState) -> Vec<Box<dyn Scene>> {
    match state {
        StockState::Listable | StockState::Full => {
            let listable = state == StockState::Listable;
            vec![Box::new(bsn! {
                @row_button(ButtonArt::NORMAL, "List", 44.0)
                @usable(listable, Color::WHITE)
                VendingAction::List(cart_index)
                on(on_vending_button)
            })]
        }
        StockState::Listed => vec![Box::new(chrome_text(
            "Listed".into(),
            11.0,
            ro_dark::STATUS_ONLINE,
        ))],
        StockState::Locked => vec![
            Box::new(glyph_icon("lock", 11.0, ro_dark::MUTED)),
            Box::new(chrome_text(
                "Can\u{2019}t sell".into(),
                11.0,
                ro_dark::MUTED,
            )),
        ],
    }
}

/// One drafted listing: its item over a live note, the quantity and price fields,
/// what it earns if all of it sells, and the remove mark.
fn listing_row(view: ListingRowView) -> impl Scene {
    let index = view.cart_index;
    let item = VendingItem {
        surface: Surface::Cart,
        index,
    };
    let color = name_color(&view.cell);
    let amount = VendingDraftField {
        cart_index: index,
        kind: DraftFieldKind::Amount,
    };
    let price = VendingDraftField {
        cart_index: index,
        kind: DraftFieldKind::Price,
    };
    bsn! {
        @item_row(item, 58.0)
        Children [
            @slot(view.cell, SLOT, ICON)
            --
            @stacked(
                name_line(view.name, 12.0, color),
                bsn! { SetupReadout::Note(index) @sub_line(String::new()) },
            )
            --
            @number_field(view.amount, QTY_DIGITS)
            amount
            TextLayout { justify: Justify::Right }
            Node { width: px(64), padding: {UiRect::axes(px(8), px(2))} }
            --
            Node {
                width: px(128),
                height: px(24),
                flex_shrink: 0.0,
                position_type: PositionType::Relative,
            }
            @ignore_picking()
            Children [
                @number_field(view.price, PRICE_DIGITS)
                price
                TextColor(ro_dark::BRONZE_LIGHT)
                TextLayout { justify: Justify::Right }
                Node { width: px(128), padding: {UiRect::new(px(8), px(22), px(2), px(2))} }
                --
                @chrome_text("z".into(), 12.0, ro_dark::ACCENT)
                Node { position_type: PositionType::Absolute, right: px(9), top: px(4) }
                --
                SetupReadout::Words(index)
                @chrome_text(String::new(), 10.0, ro_dark::MUTED)
                TextLayout { linebreak: LineBreak::NoWrap }
                Node { position_type: PositionType::Absolute, right: px(2), top: px(26) }
            ]
            --
            Node { width: px(96), flex_shrink: 0.0, justify_content: JustifyContent::FlexEnd }
            @ignore_picking()
            Children [
                SetupReadout::Total(index)
                @chrome_text(String::new(), 12.0, ro_dark::STRONG)
            ]
            --
            @ghost_button("close")
            VendingAction::Unlist(index)
            on(on_vending_button)
        ]
    }
}

fn setup_footer(cost: String) -> impl Scene {
    bsn! {
        Node { align_items: AlignItems::Center, column_gap: px(18) }
        @ignore_picking()
        Children [
            @wide_button(ButtonArt::NORMAL, "Back to Cart", 112.0)
            VendingAction::CancelSetup
            on(on_vending_button)
            --
            @stat("If everything sells", bsn! {
                SetupReadout::Potential
                @chrome_text(String::new(), 16.0, ro_dark::BRONZE_LIGHT)
            })
            --
            @stat("Opening cost", chrome_text(cost, 13.0, ro_dark::STRONG))
            --
            Node {
                flex_grow: 1.0,
                flex_basis: px(0),
                min_width: px(0),
                justify_content: JustifyContent::FlexEnd,
            }
            @ignore_picking()
            Children [
                SetupReadout::Status
                @chrome_text(String::new(), 12.0, ro_dark::STATUS_FULL)
            ]
            --
            VendingOpenButton
            @wide_button(ButtonArt::PRIMARY, "Open Shop", 140.0)
            VendingAction::OpenShop
            on(on_vending_button)
        ]
    }
}

/// The seller's body: the live sign and status, the stock beside the latest sales,
/// then the takings and Close Shop. `scroll` is the stock's and the sales'.
pub(super) fn seller(view: SellerView, scroll: [Vec2; 2]) -> impl Scene {
    let [stock_scroll, sales_scroll] = scroll;
    let (status, tone) = if view.closing {
        ("Closing", ro_dark::STATUS_BUSY)
    } else {
        ("Open", ro_dark::STATUS_ONLINE)
    };
    let glow = ro_dark::FROST_GLOW.with_alpha(0.45);
    let stock_count = format!("{} of {} still selling", view.selling(), view.stock.len());
    let sales_count = match view.sales.len() {
        0 => String::new(),
        shown => format!("Latest {shown}"),
    };
    let earned = zeny(view.earned);
    let zeny_now = zeny(view.zeny.into());
    let left = counted(view.left() as usize, "item");
    let no_sales = view.sales.is_empty().then_some("No sales yet");
    let closing = close_controls(view.ask_close, view.closing, view.error);
    let stock: Vec<_> = view.stock.into_iter().map(shop_stock_row).collect();
    let sales: Vec<_> = view
        .sales
        .into_iter()
        .enumerate()
        .map(|(position, sale)| sale_row(sale, position == 0))
        .collect();
    let heads = vec![column_head("EACH", 90.0), column_head("EARNED", 100.0)];
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(14) }
        @ignore_picking()
        Children [
            Node { height: px(44), align_items: AlignItems::Center, column_gap: px(16) }
            @ignore_picking()
            Children [
                @shop_sign(sign_title(view.title), 300.0, glow)
                @ignore_picking()
                --
                Node { align_items: AlignItems::Center, column_gap: px(6) }
                @ignore_picking()
                Children [
                    Node { width: px(7), height: px(7), border_radius: BorderRadius::MAX }
                    BackgroundColor(tone)
                    @ignore_picking()
                    --
                    @chrome_text(status.into(), 12.0, tone)
                ]
                --
                Node { flex_grow: 1.0 }
                @ignore_picking()
                --
                @offline_toggle()
            ]
            --
            Node { column_gap: px(20), align_items: AlignItems::FlexStart }
            @ignore_picking()
            Children [
                Node { width: px(500), flex_shrink: 0.0, flex_direction: FlexDirection::Column }
                @ignore_picking()
                Children [
                    @panel_head("STOCK", stock_count, ro_dark::STRONG)
                    --
                    @inset()
                    Node { height: px(SHOP_LIST), flex_direction: FlexDirection::Column }
                    @ignore_picking()
                    Children [
                        @list_head(SLOT, heads)
                        --
                        @scroll_rows(VendingScroll::Stock, stock_scroll, stock, None)
                    ]
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
                    @panel_head("SALES", sales_count, ro_dark::STRONG)
                    --
                    @inset()
                    Node { height: px(SHOP_LIST), flex_direction: FlexDirection::Column }
                    @ignore_picking()
                    Children [ @scroll_rows(VendingScroll::Sales, sales_scroll, sales, no_sales) ]
                ]
            ]
            --
            Node { min_height: px(40), align_items: AlignItems::Center, column_gap: px(24) }
            @ignore_picking()
            Children [
                @stat("Earned this session", chrome_text(earned, 18.0, ro_dark::BRONZE_LIGHT))
                --
                @stat("Zeny now", chrome_text(zeny_now, 13.0, ro_dark::STRONG))
                --
                @stat("Items left", chrome_text(left, 13.0, ro_dark::STRONG))
                --
                Node {
                    flex_grow: 1.0,
                    flex_basis: px(0),
                    min_width: px(0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::FlexEnd,
                    column_gap: px(10),
                }
                @ignore_picking()
                Children [ {closing} ]
            ]
        ]
    }
}

/// NOTE: drawn switched off and inert, because the protocol has no offline vending.
/// Wire it to a command once aesir can keep a shop open after its owner logs out.
fn offline_toggle() -> impl Scene {
    bsn! {
        Node { align_items: AlignItems::Center, column_gap: px(10) }
        @ignore_picking()
        Children [
            Node {
                width: px(40),
                height: px(22),
                padding: px(2),
                border: px(1),
                border_radius: BorderRadius::all(px(11)),
            }
            BackgroundColor(ro_dark::BAR_TROUGH)
            BorderColor::all(ro_dark::DISABLED_STROKE)
            @ignore_picking()
            Children [
                Node { width: px(16), height: px(16), border_radius: BorderRadius::MAX }
                BackgroundColor(ro_dark::DISABLED_LABEL)
                @ignore_picking()
            ]
            --
            Node { flex_direction: FlexDirection::Column }
            @ignore_picking()
            Children [
                @chrome_text("Stay open offline".into(), 12.0, ro_dark::DISABLED_LABEL)
                --
                @chrome_text("Keeps selling after you log out".into(), 11.0, ro_dark::FAINT)
            ]
        ]
    }
}

/// One listing of the open shop: how much of it sold, its price and what it earned.
fn shop_stock_row(view: ShopStockView) -> impl Scene {
    let item = VendingItem {
        surface: Surface::Shop,
        index: view.index,
    };
    let sold_out = view.sold >= view.total;
    let color = if sold_out {
        ro_dark::MUTED
    } else {
        name_color(&view.cell)
    };
    let tag = sold_out.then(|| {
        EntityScene(chip(
            "Sold out".into(),
            ro_dark::STATUS_ONLINE.with_alpha(0.18),
            ro_dark::STATUS_ONLINE,
        ))
    });
    let fill = view.sold as f32 / view.total.max(1) as f32 * 100.0;
    let sold = format!(
        "{} of {} sold",
        thousands(view.sold.into()),
        thousands(view.total.into())
    );
    let earned = u64::from(view.sold) * u64::from(view.price);
    let (earned, earned_color) = if earned > 0 {
        (zeny(earned), ro_dark::STATUS_ONLINE)
    } else {
        ("\u{2014}".to_string(), ro_dark::MUTED)
    };
    let price = zeny(view.price.into());
    bsn! {
        @item_row(item, 52.0)
        Children [
            @slot(view.cell, SLOT, ICON)
            --
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                flex_basis: px(0),
                min_width: px(0),
                row_gap: px(4),
                overflow: {Overflow::clip()},
            }
            @ignore_picking()
            Children [
                Node { align_items: AlignItems::Center, column_gap: px(8) }
                @ignore_picking()
                Children [ @name_line(view.name, 12.0, color) -- {tag} ]
                --
                Node { align_items: AlignItems::Center, column_gap: px(8) }
                @ignore_picking()
                Children [
                    Node {
                        width: px(120),
                        height: px(6),
                        flex_shrink: 0.0,
                        border_radius: BorderRadius::all(px(3)),
                    }
                    BackgroundColor(ro_dark::BAR_TROUGH)
                    @ignore_picking()
                    Children [
                        Node {
                            width: {Val::Percent(fill)},
                            height: percent(100),
                            border_radius: BorderRadius::all(px(3)),
                        }
                        BackgroundColor(ro_dark::JOB_EXP)
                        @ignore_picking()
                    ]
                    --
                    @sub_line(sold)
                ]
            ]
            --
            @figure(price, 90.0, ro_dark::BRONZE_LIGHT)
            --
            @figure(earned, 100.0, earned_color)
        ]
    }
}

/// One reported sale. The newest is washed green.
fn sale_row(view: SaleView, fresh: bool) -> impl Scene {
    let fill = if fresh {
        ro_dark::STATUS_ONLINE.with_alpha(0.1)
    } else {
        Color::NONE
    };
    let gain = format!("+{}", zeny(view.zeny.into()));
    let line = vec![vec![
        (ro_dark::STRONG, view.buyer),
        (ro_dark::LABEL, format!(" bought {}", view.what)),
    ]];
    bsn! {
        Node {
            flex_shrink: 0.0,
            column_gap: px(10),
            padding: {UiRect::axes(px(10), px(7))},
            border: {UiRect::bottom(px(1))},
        }
        BackgroundColor(fill)
        BorderColor::all(ro_dark::INSET_RULE)
        @ignore_picking()
        Children [
            Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0) }
            @ignore_picking()
            Children [ @colored_lines(line) ]
            --
            @chrome_text(gain, 12.0, ro_dark::STATUS_ONLINE)
        ]
    }
}

/// Close Shop, or the prompt it raises, after the last rejection if there is one.
fn close_controls(ask: bool, closing: bool, error: Option<String>) -> Vec<Box<dyn Scene>> {
    let idle = !closing;
    let mut controls: Vec<Box<dyn Scene>> = Vec::new();
    if let Some(error) = error {
        controls.push(Box::new(chrome_text(error, 12.0, ro_dark::STATUS_FULL)));
    }
    if !ask {
        controls.push(Box::new(bsn! {
            @wide_button(ButtonArt::NORMAL, "Close Shop", 112.0)
            @usable(idle, DANGER_TINT)
            VendingAction::AskClose
            on(on_vending_button)
        }));
        return controls;
    }
    controls.push(Box::new(chrome_text(
        "Close the shop? Unsold items go back to your cart.".into(),
        12.0,
        ro_dark::DANGER_GLOW,
    )));
    controls.push(Box::new(bsn! {
        @row_button(ButtonArt::NORMAL, "Keep Open", 76.0)
        VendingAction::KeepOpen
        on(on_vending_button)
    }));
    controls.push(Box::new(bsn! {
        @row_button(ButtonArt::NORMAL, "Close Shop", 84.0)
        @usable(idle, DANGER_TINT)
        VendingAction::CloseShop
        on(on_vending_button)
    }));
    controls
}

/// The buyer's body: the vendor's sign, the listings beside the basket, then the
/// buyer's own zeny and weight. `scroll` is the listings' and the basket's.
pub(super) fn buyer(view: BuyerView, scroll: [Vec2; 2]) -> impl Scene {
    let [list_scroll, basket_scroll] = scroll;
    let on_sale = view.rows.iter().filter(|row| row.left > 0).count();
    let count = counted(on_sale, "listing");
    let title = view.title.clone();
    let vendor = view.vendor.clone().unwrap_or_default();
    let banner = view
        .change
        .clone()
        .map(|text| EntityScene(change_banner(text)));
    let no_rows = view.rows.is_empty().then_some("Nothing is for sale");
    let rows: Vec<_> = view.rows.iter().cloned().map(buy_row).collect();
    let zeny_now = zeny(view.zeny.into());
    let weight = format!(
        "{} / {}",
        thousands(view.weight.into()),
        thousands(view.max_weight.into())
    );
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(14) }
        @ignore_picking()
        Children [
            Node { height: px(40), align_items: AlignItems::Center, column_gap: px(16) }
            @ignore_picking()
            Children [
                @shop_sign(sign_title(title), 300.0, Color::NONE)
                @ignore_picking()
                --
                @chrome_text(vendor, 12.0, ro_dark::HERO)
                Node { flex_grow: 1.0 }
                --
                // NOTE: disabled, because the client has no whisper. Point it at the
                // chat's whisper target once that exists.
                @row_button(ButtonArt::NORMAL, "Whisper", 76.0)
                @usable(false, Color::WHITE)
            ]
            --
            Node { column_gap: px(20), align_items: AlignItems::FlexStart }
            @ignore_picking()
            Children [
                Node {
                    width: px(552),
                    height: px(BUY_COLUMN),
                    flex_shrink: 0.0,
                    flex_direction: FlexDirection::Column,
                }
                @ignore_picking()
                Children [
                    Node {
                        height: px(22),
                        flex_shrink: 0.0,
                        margin: {UiRect::bottom(px(6))},
                        align_items: AlignItems::Center,
                        column_gap: px(8),
                    }
                    @ignore_picking()
                    Children [
                        @overline("FOR SALE".into(), ro_dark::ACCENT)
                        --
                        @chrome_text(count, 12.0, ro_dark::STRONG)
                        Node { flex_grow: 1.0 }
                        --
                        @chrome_text("Right-click an item for details".into(), 11.0, ro_dark::MUTED)
                        --
                        @ghost_button("refresh")
                        VendingAction::Refresh
                        on(on_vending_button)
                    ]
                    --
                    {banner}
                    --
                    @inset()
                    Node { flex_grow: 1.0, min_height: px(0), flex_direction: FlexDirection::Column }
                    @ignore_picking()
                    Children [ @scroll_rows(VendingScroll::ForSale, list_scroll, rows, no_rows) ]
                ]
                --
                @basket(view, basket_scroll)
            ]
            --
            Node { align_items: AlignItems::Center, column_gap: px(24) }
            @ignore_picking()
            Children [
                @labelled("Your zeny", zeny_now, 15.0, ro_dark::BRONZE_LIGHT)
                --
                @labelled("Weight", weight, 12.0, ro_dark::STRONG)
                --
                Node { flex_grow: 1.0 }
                @ignore_picking()
                --
                @chrome_text(
                    "Sales are final \u{b7} The seller is paid the moment you buy".into(),
                    11.0,
                    ro_dark::MUTED,
                )
            ]
        ]
    }
}

fn labelled(label: &'static str, value: String, size: f32, color: Color) -> impl Scene {
    bsn! {
        Node { align_items: AlignItems::Baseline, column_gap: px(8), flex_shrink: 0.0 }
        @ignore_picking()
        Children [
            @chrome_text(label.into(), 12.0, ro_dark::MUTED)
            --
            @chrome_text(value, size, color)
        ]
    }
}

/// What a refreshed shop took out of the basket, until the buyer acknowledges it.
fn change_banner(text: String) -> impl Scene {
    bsn! {
        Node {
            flex_shrink: 0.0,
            margin: {UiRect::bottom(px(8))},
            padding: {UiRect::new(px(12), px(10), px(7), px(7))},
            align_items: AlignItems::Center,
            column_gap: px(10),
            border: px(1),
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor({ro_dark::STATUS_BUSY.with_alpha(0.12)})
        BorderColor::all(ro_dark::STATUS_BUSY.with_alpha(0.45))
        @ignore_picking()
        Children [
            @diamond(8.0, ro_dark::STATUS_BUSY, ro_dark::STATUS_BUSY)
            --
            Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0) }
            @ignore_picking()
            Children [ @chrome_text(text, 12.0, ro_dark::BRONZE_LIGHT) ]
            --
            @row_button(ButtonArt::NORMAL, "OK", 40.0)
            VendingAction::AckChange
            on(on_vending_button)
        ]
    }
}

/// One listing: its item, what is left, the unit price, then the quantity controls.
fn buy_row(view: BuyRowView) -> impl Scene {
    let item = VendingItem {
        surface: Surface::Listing,
        index: view.index,
    };
    let color = if view.left == 0 {
        ro_dark::MUTED
    } else {
        name_color(&view.cell)
    };
    let tag = (view.in_cart > 0).then(|| {
        EntityScene(chip(
            format!("{} in cart", thousands(view.in_cart.into())),
            ro_dark::PRIMARY,
            ro_dark::CHECK_MARK,
        ))
    });
    let price_words = words(view.price.into());
    let words_color = if u64::from(view.price) >= CONFIRM_FROM {
        ro_dark::BRONZE_LIGHT
    } else {
        ro_dark::MUTED
    };
    let price = zeny(view.price.into());
    let control = buy_control(view.index, view.room(), view.left);
    bsn! {
        @item_row(item, 56.0)
        Children [
            @slot(view.cell, BUY_SLOT, BUY_ICON)
            --
            @stacked(
                bsn! {
                    Node { align_items: AlignItems::Center, column_gap: px(6) }
                    @ignore_picking()
                    Children [ @name_line(view.name, 13.0, color) -- {tag} ]
                },
                sub_line(view.sub),
            )
            --
            Node {
                width: px(104),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
            }
            @ignore_picking()
            Children [
                @chrome_text(price, 13.0, ro_dark::BRONZE_LIGHT)
                --
                @chrome_text(price_words, 10.0, words_color)
            ]
            --
            Node {
                width: px(160),
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

/// The stepper, quantity field and Add button of a listing, or why it has none.
fn buy_control(index: u32, room: u32, left: u32) -> Vec<Box<dyn Scene>> {
    if room == 0 {
        let why = if left == 0 { "Sold out" } else { "All in cart" };
        return vec![Box::new(chrome_text(why.into(), 11.0, ro_dark::MUTED))];
    }
    vec![
        Box::new(step_button("minus", VendingAction::RowDec(index))),
        Box::new(bsn! {
            @number_field("1".to_string(), QTY_DIGITS)
            VendingQtyField(index)
            TextLayout { justify: Justify::Center }
            Node { width: px(46) }
        }),
        Box::new(step_button("plus", VendingAction::RowInc(index))),
        Box::new(bsn! {
            @row_button(ButtonArt::NORMAL, "Add", 44.0)
            VendingAction::AddRow(index)
            on(on_vending_button)
        }),
    ]
}

/// The basket column: its caption, the staged lines, then the totals and Buy.
fn basket(view: BuyerView, scroll: Vec2) -> impl Scene {
    let count = match view.lines.len() {
        0 => String::new(),
        lines => counted(lines, "line"),
    };
    let hint = view
        .lines
        .is_empty()
        .then_some("Add items from the shop. Nothing is bought until you press Buy.");
    let lines: Vec<_> = view.lines.iter().cloned().map(basket_line).collect();
    bsn! {
        @inset()
        Node {
            width: px(324),
            height: px(BUY_COLUMN),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
        }
        @ignore_picking()
        Children [
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
                @overline("YOUR CART".into(), ro_dark::ACCENT)
                --
                @chrome_text(count, 12.0, ro_dark::STRONG)
            ]
            --
            @scroll_rows(VendingScroll::Basket, scroll, lines, hint)
            --
            @basket_foot(view)
        ]
    }
}

fn basket_line(view: BasketLineView) -> impl Scene {
    let item = VendingItem {
        surface: Surface::Listing,
        index: view.index,
    };
    let index = view.index;
    let color = name_color(&view.cell);
    let qty = format!("\u{d7}{}", thousands(view.qty.into()));
    bsn! {
        @item_row(item, 44.0)
        Children [
            @slot(view.cell, LINE_SLOT, LINE_ICON)
            --
            @stacked(name_line(view.name, 12.0, color), sub_line(zeny(view.total)))
            --
            @chrome_text(qty, 12.0, ro_dark::STRONG)
            --
            @ghost_button("close")
            VendingAction::RemoveLine(index)
            on(on_vending_button)
        ]
    }
}

/// The "check the zeros" prompt when it is up, the totals, what stops the purchase,
/// then Buy.
fn basket_foot(view: BuyerView) -> impl Scene {
    let confirm = view.confirm.then(|| EntityScene(confirm_box(view.total)));
    let (after, after_color) = match u64::from(view.zeny).checked_sub(view.total) {
        Some(after) => (zeny(after), ro_dark::STRONG),
        None => (
            format!("\u{2212}{}", zeny(view.total - u64::from(view.zeny))),
            ro_dark::STATUS_FULL,
        ),
    };
    let totals = vec![
        summary_row("Total", zeny(view.total), ro_dark::BRONZE_LIGHT),
        summary_row("Zeny after", after, after_color),
    ];
    let problem = view
        .error
        .clone()
        .or_else(|| view.short().then(|| "Not enough zeny.".to_string()))
        .map(|text| EntityScene(chrome_text(text, 11.0, ro_dark::STATUS_FULL)));
    let cta = if view.lines.is_empty() {
        "Buy".to_string()
    } else {
        format!("Buy for {}", zeny(view.total))
    };
    let can_buy = view.can_buy();
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
            {confirm}
            --
            {totals}
            --
            {problem}
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
            @usable(can_buy, Color::WHITE)
            VendingAction::Buy
            on(on_vending_button)
        ]
    }
}

fn summary_row(label: &'static str, value: String, ink: Color) -> impl Scene {
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

/// The prompt a large total raises: the amount again in words, then Back and Buy Now.
fn confirm_box(total: u64) -> impl Scene {
    let paying = format!("You\u{2019}re about to pay {}", zeny(total));
    let spelled = format!(
        "That\u{2019}s {}. Check the zeros before you buy.",
        words(total)
    );
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(2),
            padding: {UiRect::axes(px(12), px(10))},
            margin: {UiRect::bottom(px(10))},
            border: px(1),
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor({ro_dark::STATUS_BUSY.with_alpha(0.1)})
        BorderColor::all(ro_dark::STATUS_BUSY.with_alpha(0.5))
        @ignore_picking()
        Children [
            @chrome_text(paying, 12.0, ro_dark::BRONZE_LIGHT)
            --
            @chrome_text(spelled, 12.0, ro_dark::BODY)
            --
            Node {
                margin: {UiRect::top(px(8))},
                justify_content: JustifyContent::FlexEnd,
                column_gap: px(8),
            }
            @ignore_picking()
            Children [
                @row_button(ButtonArt::NORMAL, "Back", 52.0)
                VendingAction::BackFromConfirm
                on(on_vending_button)
                --
                @row_button(ButtonArt::PRIMARY, "Buy Now", 76.0)
                VendingAction::BuyNow
                on(on_vending_button)
            ]
        ]
    }
}

/// What a closed shop left behind, with the way back to the cart or into a new setup.
pub(super) fn closed(view: ClosedView) -> impl Scene {
    let unsold = match view.unsold {
        0 => "Everything sold.".to_string(),
        unsold => format!(
            "{} went back to your cart.",
            counted(unsold as usize, "unsold item")
        ),
    };
    let summary = format!("You earned {}. {unsold}", zeny(view.earned));
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(8) }
        @ignore_picking()
        Children [
            @display_text("Shop closed.".into(), 18.0, ro_dark::TITLE)
            --
            @chrome_text(summary, 13.0, ro_dark::BODY)
            --
            Node {
                margin: {UiRect::top(px(8))},
                justify_content: JustifyContent::FlexEnd,
                column_gap: px(10),
            }
            @ignore_picking()
            Children [
                @wide_button(ButtonArt::NORMAL, "Back to Cart", 112.0)
                VendingAction::BackToCart
                on(on_vending_button)
                --
                @wide_button(ButtonArt::PRIMARY, "Set Up Again", 124.0)
                VendingAction::SetUpAgain
                on(on_vending_button)
            ]
        ]
    }
}
