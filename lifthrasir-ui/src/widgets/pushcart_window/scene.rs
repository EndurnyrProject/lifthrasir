//! Idiomatic BSN chrome for the pushcart window (mirrors the inventory/shop
//! windows). [`window`] builds the persistent chrome — root, titlebar, and an
//! empty body region — as one `bsn!` tree; [`body`] renders one of two mount
//! states and is respawned by [`rebuild_body`](super::rebuild_body) on every
//! mount-state / [`Cart`](game_engine::domain::cart::Cart) change.
//!
//! The mounted [`body`] projects the live `Inventory` + `Cart` into two sprite-
//! well panes, a detail strip (selected sprite/name/type + quantity stepper +
//! Move button), and a footer of body/cart weight meters, cart slot usage, and
//! zeny. Every view-model is a pure function of owned data, prepared before the
//! `bsn!` block, so the projection is unit-testable without spawning the tree.

use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui_widgets::{Activate, ControlOrientation, ScrollArea};
use bevy_feathers::controls::{FeathersButton, FeathersScrollbar};
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor, ThemeTextColor};
use game_engine::domain::assets::item_icon_path;
use game_engine::domain::cart::Cart;
use game_engine::domain::entities::character::components::status::CharacterStatus;
use game_engine::domain::inventory::Inventory;
use game_engine::infrastructure::item::ItemDb;
use net_contract::events::CartMountRejection;

use crate::theme;
use crate::theme::feathers_theme::{
    TOKEN_TEXT, TOKEN_TITLEBAR_BG, TOKEN_WINDOW_BG, TOKEN_WINDOW_BORDER,
};
use crate::widgets::draggable::px_or_zero;

use super::{
    move_enabled, on_cell_click, on_mount_toggle, on_move, on_qty_step, CartCell, CartWindowBody,
    CartWindowRoot, CartWindowTitlebar, MountToggleButton, QtyButton, Side, CART_MAX_SLOTS,
};

const WINDOW_LEFT: f32 = 360.0;
const WINDOW_TOP: f32 = 130.0;
const WINDOW_WIDTH: f32 = 520.0;
const CELL_SIZE: f32 = 32.0;
/// Fixed height of each pane's scrollable grid, so the window never grows with
/// content — the grid scrolls internally instead.
const PANE_HEIGHT: f32 = 200.0;

/// Spawn the whole window as one scene and parent it under `parent` with a single
/// insert.
pub fn build(commands: &mut Commands, parent: Entity) {
    commands.spawn_scene(window()).insert(ChildOf(parent));
}

fn window() -> impl Scene {
    bsn! {
        CartWindowRoot
        Node {
            position_type: PositionType::Absolute,
            left: px(WINDOW_LEFT),
            top: px(WINDOW_TOP),
            width: px(WINDOW_WIDTH),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
            border: px(1),
            border_radius: BorderRadius::all(px(13)),
        }
        ThemeBackgroundColor({TOKEN_WINDOW_BG})
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        Visibility::Hidden
        Pickable
        Children [ titlebar(), body_container() ]
    }
}

fn titlebar() -> impl Scene {
    bsn! {
        CartWindowTitlebar
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
        on(on_titlebar_drag)
        Children [
            glyph_icon("cart", 16.0, theme::GOLD),
            (
                Text("Pushcart")
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/cinzel.ttf"),
                    font_size: {FontSize::Px(15.0)},
                }
                ThemeTextColor({TOKEN_TEXT})
                Node { flex_grow: 1.0 }
                ignore_picking()
            ),
            (
                @FeathersButton { @caption: bsn! { glyph_icon("close", 13.0, theme::TEXT_DIM) } }
                Node { width: px(22), height: px(22) }
                on(on_close)
            ),
        ]
    }
}

/// The (initially empty) body region; [`body`] fills it via `rebuild_body`.
fn body_container() -> impl Scene {
    bsn! {
        CartWindowBody
        Node {
            flex_direction: FlexDirection::Column,
            padding: {UiRect { left: Val::Px(14.0), right: Val::Px(14.0), top: Val::Px(12.0), bottom: Val::Px(14.0) }},
        }
        ignore_picking()
    }
}

/// The swappable body: the mount prompt when the local player has no cart, or
/// the mounted Bag<->Cart view when they do. Exactly one branch renders. The
/// mounted branch needs the local player's [`CharacterStatus`] for the footer;
/// its absence (guarded loudly upstream in [`rebuild_body`](super::rebuild_body))
/// collapses the mounted view to nothing rather than fabricating a footer.
pub fn body(
    mounted: bool,
    inventory: &Inventory,
    cart: &Cart,
    cart_ui: &super::CartUi,
    status: Option<&CharacterStatus>,
    item_db: Option<&ItemDb>,
) -> impl Scene {
    let prompt = (!mounted).then(|| EntityScene(mount_prompt(cart_ui.mount_error)));
    let mounted_view = mounted
        .then_some(status)
        .flatten()
        .map(|status| EntityScene(mounted_body(inventory, cart, cart_ui, status, item_db)));
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(12) }
        ignore_picking()
        Children [ {prompt}, {mounted_view} ]
    }
}

/// Shown when the local player has no cart: a line of copy, a single
/// "Mount Pushcart" button that sends `MountCart { mount: true }`, and — after a
/// rejected mount — a warning line explaining why the last attempt failed.
fn mount_prompt(error: Option<CartMountRejection>) -> impl Scene {
    let hint =
        error.map(|reason| EntityScene(chrome_text(mount_error_text(reason), 11.0, theme::WARN)));
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(10),
            padding: {UiRect::vertical(px(10))},
        }
        ignore_picking()
        Children [
            chrome_text("You have no pushcart mounted.".to_string(), 12.0, theme::TEXT_DIM),
            (
                @FeathersButton { @caption: bsn! { chrome_text("Mount Pushcart".to_string(), 13.0, theme::TEXT) } }
                template_value(MountToggleButton { mount: true })
                Node {
                    height: px(32),
                    padding: {UiRect::horizontal(px(18))},
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(px(7)),
                }
                BackgroundColor(theme::EMERALD_INK)
                on(on_mount_toggle)
            ),
            {hint},
        ]
    }
}

/// The warning copy shown under the mount button for each rejection reason.
fn mount_error_text(reason: CartMountRejection) -> String {
    match reason {
        CartMountRejection::SkillNotLearned => "You have not learned Pushcart.".to_string(),
        CartMountRejection::AlreadyMounted => "A cart is already mounted.".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Mounted body: Bag | mover | Cart panes, a detail strip, and the footer meters.
// `bsn!` scenes own their data, so every view-model below is prepared as owned
// values *before* entering a `bsn!` block — no live resource reference crosses
// into one.
// ---------------------------------------------------------------------------

/// One pane cell's owned view-model. `side` + `index` become the [`CartCell`] so
/// a click knows which container and slot it selected.
pub(crate) struct CellView {
    side: Side,
    index: u16,
    icon: Option<String>,
    amount: u16,
    refine: u8,
    selected: bool,
}

/// The selected item's owned view-model for the detail strip. `type_label` is
/// `None` for cart items, whose DTO carries no `item_type`.
pub(crate) struct DetailView {
    icon: Option<String>,
    name: String,
    type_label: Option<String>,
}

/// The footer's owned view-model: both weight meters, cart slot usage, and zeny.
pub(crate) struct FooterView {
    body_weight: u32,
    body_max: u32,
    cart_weight: u32,
    cart_max: u32,
    cart_slots: usize,
    zeny: u32,
}

fn item_icon(item_db: Option<&ItemDb>, nameid: u32, identified: bool) -> Option<String> {
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

/// The Bag pane's cells: every non-worn inventory item (worn equipment lives in
/// the equipment window, not the bag). A missing `ItemDb` entry yields a cell
/// with no icon — never a panic.
pub(crate) fn bag_cells(
    inventory: &Inventory,
    item_db: Option<&ItemDb>,
    selected: Option<(Side, u16)>,
) -> Vec<CellView> {
    inventory
        .iter()
        .filter(|item| !item.is_equipped())
        .map(|item| CellView {
            side: Side::Bag,
            index: item.index,
            icon: item_icon(item_db, item.item_id, item.identified),
            amount: item.amount,
            refine: item.refine,
            selected: selected == Some((Side::Bag, item.index)),
        })
        .collect()
}

/// The Cart pane's cells, in cart-index order.
pub(crate) fn cart_cells(
    cart: &Cart,
    item_db: Option<&ItemDb>,
    selected: Option<(Side, u16)>,
) -> Vec<CellView> {
    cart.iter()
        .map(|item| {
            let index = item.index as u16;
            CellView {
                side: Side::Cart,
                index,
                icon: item_icon(item_db, item.nameid, item.identified),
                amount: item.amount.min(u16::MAX as u32) as u16,
                refine: item.refine.min(u8::MAX as u32) as u8,
                selected: selected == Some((Side::Cart, index)),
            }
        })
        .collect()
}

/// The detail view-model for the current selection, resolved from whichever
/// container it belongs to. `None` when nothing is selected or the slot has
/// vanished (a stale selection after a server move).
pub(crate) fn detail_view(
    selected: Option<(Side, u16)>,
    inventory: &Inventory,
    cart: &Cart,
    item_db: Option<&ItemDb>,
) -> Option<DetailView> {
    let (side, index) = selected?;
    match side {
        Side::Bag => {
            let item = inventory.get(index)?;
            Some(DetailView {
                icon: item_icon(item_db, item.item_id, item.identified),
                name: item_name(item_db, item.item_id, item.identified),
                type_label: Some(item.type_label().to_string()),
            })
        }
        Side::Cart => {
            let item = cart.get(index)?;
            Some(DetailView {
                icon: item_icon(item_db, item.nameid, item.identified),
                name: item_name(item_db, item.nameid, item.identified),
                type_label: None,
            })
        }
    }
}

pub(crate) fn footer_view(cart: &Cart, status: &CharacterStatus) -> FooterView {
    FooterView {
        body_weight: status.weight,
        body_max: status.max_weight,
        cart_weight: cart.current_weight(),
        cart_max: cart.max_weight(),
        cart_slots: cart.len(),
        zeny: status.zeny,
    }
}

/// Shown while the cart is mounted: the Bag<->Cart panes, the detail strip, the
/// footer meters, an optional unmount hint, and the Unmount button. The unmount
/// button stays clickable but its handler drops a non-empty unmount; the dimmed
/// look here mirrors that guard.
fn mounted_body(
    inventory: &Inventory,
    cart: &Cart,
    cart_ui: &super::CartUi,
    status: &CharacterStatus,
    item_db: Option<&ItemDb>,
) -> impl Scene {
    let bag = bag_cells(inventory, item_db, cart_ui.selected);
    let cart_items = cart_cells(cart, item_db, cart_ui.selected);
    let detail = detail_view(cart_ui.selected, inventory, cart, item_db);
    let footer_data = footer_view(cart, status);
    let can_move = move_enabled(cart_ui.selected, cart_ui.qty, inventory, cart);
    let cart_empty = cart.is_empty();
    let qty = cart_ui.qty;

    let hint = (!cart_empty).then(|| {
        EntityScene(chrome_text(
            "Empty the cart before unmounting.".to_string(),
            11.0,
            theme::WARN,
        ))
    });
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(12) }
        ignore_picking()
        Children [
            panes_row(bag, cart_items),
            detail_strip(detail, qty, can_move),
            footer(footer_data),
            {hint},
            unmount_button(cart_empty),
        ]
    }
}

fn unmount_button(cart_empty: bool) -> impl Scene {
    let label_color = if cart_empty {
        theme::TEXT
    } else {
        theme::TEXT_FAINT
    };
    bsn! {
        @FeathersButton { @caption: bsn! { chrome_text("Unmount".to_string(), 12.0, label_color) } }
        template_value(MountToggleButton { mount: false })
        Node {
            height: px(30),
            align_self: AlignSelf::FlexStart,
            padding: {UiRect::horizontal(px(16))},
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(px(7)),
        }
        BackgroundColor(theme::FIELD)
        on(on_mount_toggle)
    }
}

/// The Bag pane, mover column, and Cart pane in a row.
fn panes_row(bag: Vec<CellView>, cart: Vec<CellView>) -> impl Scene {
    let bag_count = bag.len();
    let cart_count = cart.len();
    bsn! {
        Node { flex_direction: FlexDirection::Row, column_gap: px(10), align_items: AlignItems::Stretch }
        ignore_picking()
        Children [
            pane("Bag".to_string(), bag_count.to_string(), bag),
            mover_column(),
            pane("Cart".to_string(), format!("{cart_count} / {CART_MAX_SLOTS}"), cart),
        ]
    }
}

/// One pane: a header (title + count) over a fixed-height, wheel-scrollable grid
/// of wrapped cells with a [`FeathersScrollbar`] pinned right. The `#grid` id is
/// scoped to this call, so both panes reuse it without collision.
fn pane(title: String, subtitle: String, cells: Vec<CellView>) -> impl Scene {
    let empty = cells.is_empty();
    let items: Vec<_> = cells.into_iter().map(cell).collect();
    let empty_msg = empty.then(|| EntityScene(muted_text("Empty".to_string())));
    bsn! {
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            min_width: px(0),
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
        }
        ignore_picking()
        Children [
            pane_head(title, subtitle),
            (
                Node {
                    height: px(PANE_HEIGHT),
                    position_type: PositionType::Relative,
                    border: px(1),
                    border_radius: BorderRadius::all(px(8)),
                }
                BackgroundColor({Color::srgba(0.0, 0.0, 0.0, 0.18)})
                BorderColor::all(theme::GOLD_FAINT)
                ignore_picking()
                Children [
                    (
                        #grid
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(0), top: px(0), right: px(0), bottom: px(0),
                            overflow: {Overflow::scroll_y()},
                            flex_direction: FlexDirection::Row,
                            flex_wrap: FlexWrap::Wrap,
                            align_content: AlignContent::FlexStart,
                            column_gap: px(6),
                            row_gap: px(6),
                            padding: {UiRect { left: Val::Px(8.0), right: Val::Px(13.0), top: Val::Px(8.0), bottom: Val::Px(8.0) }},
                        }
                        ScrollArea
                        Pickable
                        Children [ {items}, {empty_msg} ]
                    ),
                    @FeathersScrollbar { @target: #grid, @orientation: {ControlOrientation::Vertical} }
                    Node {
                        position_type: PositionType::Absolute,
                        right: px(3),
                        top: px(4),
                        bottom: px(4),
                        width: px(6),
                    }
                ]
            ),
        ]
    }
}

fn pane_head(title: String, subtitle: String) -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
        }
        ignore_picking()
        Children [
            chrome_text(title, 12.0, theme::TEXT),
            chrome_text(subtitle, 10.0, theme::TEXT_FAINT),
        ]
    }
}

/// The between-panes flow indicator: a bidirectional pair of chevrons. Purely
/// decorative (the Move button in the detail strip performs the move); it never
/// swallows clicks.
fn mover_column() -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: px(8),
            width: px(24),
            flex_shrink: 0.0,
        }
        ignore_picking()
        Children [
            glyph_icon("chevr", 16.0, theme::TEXT_DIM),
            glyph_icon("chevl", 16.0, theme::TEXT_DIM),
        ]
    }
}

/// One pane cell: a bordered icon well carrying its [`CartCell`], with amount and
/// refine badges baked in. Selection highlight is baked at build time.
fn cell(view: CellView) -> impl Scene {
    let (bg, border) = if view.selected {
        (theme::EMERALD_INK, theme::EMERALD)
    } else {
        (theme::FIELD, theme::GOLD_FAINT)
    };
    let icon = view.icon.map(|path| EntityScene(cell_icon(path)));
    let amount = (view.amount > 1).then(|| EntityScene(amount_badge(view.amount.to_string())));
    let refine = (view.refine > 0).then(|| EntityScene(refine_badge(format!("+{}", view.refine))));
    bsn! {
        template_value(CartCell { side: view.side, index: view.index })
        Node {
            width: px(CELL_SIZE),
            height: px(CELL_SIZE),
            position_type: PositionType::Relative,
            border: px(1),
            border_radius: BorderRadius::all(px(5)),
        }
        BackgroundColor(bg)
        BorderColor::all(border)
        Pickable
        on(on_cell_click)
        Children [ {icon}, {amount}, {refine} ]
    }
}

/// A contained item icon filling its cell.
fn cell_icon(path: String) -> impl Scene {
    bsn! {
        ImageNode { image: {path} }
        Node {
            position_type: PositionType::Absolute,
            left: px(0), top: px(0), right: px(0), bottom: px(0),
            width: percent(100),
            height: percent(100),
        }
        ignore_picking()
    }
}

/// The stack-amount badge, pinned to the cell's bottom-right.
fn amount_badge(text: String) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(9.0)},
        }
        TextColor(theme::TEXT)
        Node { position_type: PositionType::Absolute, right: px(1), bottom: px(0) }
        ignore_picking()
    }
}

/// The refine badge, pinned to the cell's top-left.
fn refine_badge(text: String) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(9.0)},
        }
        TextColor(theme::GOLD)
        Node { position_type: PositionType::Absolute, left: px(1), top: px(0) }
        ignore_picking()
    }
}

/// The detail strip: the selected item's sprite/name/type, the quantity stepper
/// bound to `CartUi.qty`, and the Move button. With nothing selected it shows a
/// prompt and a disabled Move button.
fn detail_strip(detail: Option<DetailView>, qty: u16, can_move: bool) -> impl Scene {
    let icon = detail
        .as_ref()
        .and_then(|view| view.icon.clone())
        .map(|path| EntityScene(cell_icon(path)));
    let name = detail
        .as_ref()
        .map(|view| view.name.clone())
        .unwrap_or_else(|| "Select an item to move".to_string());
    let name_color = if detail.is_some() {
        theme::TEXT
    } else {
        theme::TEXT_DIM
    };
    let type_label = detail
        .as_ref()
        .and_then(|view| view.type_label.clone())
        .map(|text| EntityScene(chrome_text(text, 10.0, theme::TEXT_DIM)));
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(10),
            padding: {UiRect::axes(px(12), px(10))},
            border: px(1),
            border_radius: BorderRadius::all(px(8)),
        }
        BackgroundColor(theme::FIELD)
        BorderColor::all(theme::GOLD_FAINT)
        ignore_picking()
        Children [
            (
                Node {
                    width: px(40),
                    height: px(40),
                    flex_shrink: 0.0,
                    position_type: PositionType::Relative,
                    border: px(1),
                    border_radius: BorderRadius::all(px(6)),
                }
                BackgroundColor({Color::srgba(0.0, 0.0, 0.0, 0.40)})
                BorderColor::all(theme::STROKE)
                ignore_picking()
                Children [ {icon} ]
            ),
            (
                Node { flex_grow: 1.0, min_width: px(0), flex_direction: FlexDirection::Column, row_gap: px(2) }
                ignore_picking()
                Children [ chrome_text(name, 12.5, name_color), {type_label} ]
            ),
            stepper(qty),
            move_button(can_move),
        ]
    }
}

/// The quantity stepper: minus / value / plus, driving `CartUi.qty`.
fn stepper(qty: u16) -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Stretch,
            height: px(30),
            flex_shrink: 0.0,
            border: px(1),
            border_radius: BorderRadius::all(px(7)),
        }
        BackgroundColor(theme::FIELD)
        BorderColor::all(theme::STROKE)
        ignore_picking()
        Children [
            stepper_button("minus", false),
            stepper_value(qty),
            stepper_button("plus", true),
        ]
    }
}

fn stepper_button(icon_name: &'static str, inc: bool) -> impl Scene {
    bsn! {
        @FeathersButton { @caption: bsn! { glyph_icon(icon_name, 12.0, theme::TEXT_DIM) } }
        template_value(QtyButton { inc })
        Node { width: px(26), height: px(28) }
        on(on_qty_step)
    }
}

fn stepper_value(qty: u16) -> impl Scene {
    bsn! {
        Text({qty.to_string()})
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(12.0)},
        }
        TextColor(theme::TEXT)
        TextLayout { justify: Justify::Center }
        Node {
            width: px(36),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
        }
        ignore_picking()
    }
}

/// The Move button: emerald when a move is allowed, dimmed field when the client
/// caps disable it. The handler re-checks the same guard, so a click on a dimmed
/// button is a no-op rather than a doomed request.
fn move_button(enabled: bool) -> impl Scene {
    let bg = if enabled {
        theme::EMERALD
    } else {
        theme::FIELD
    };
    bsn! {
        @FeathersButton { @caption: bsn! { chrome_text("Move".to_string(), 12.0, theme::TEXT) } }
        Node {
            height: px(30),
            flex_shrink: 0.0,
            padding: {UiRect::horizontal(px(16))},
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(px(7)),
        }
        BackgroundColor(bg)
        on(on_move)
    }
}

/// The footer: the body-weight meter, the cart-weight meter, cart slot usage,
/// and the zeny balance.
fn footer(view: FooterView) -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(14),
            padding: {UiRect::axes(px(2), px(4))},
            border: {UiRect { top: Val::Px(1.0), ..default() }},
        }
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        ignore_picking()
        Children [
            meter("Weight".to_string(), view.body_weight, view.body_max, theme::EMERALD),
            meter("Cart".to_string(), view.cart_weight, view.cart_max, theme::GOLD),
            slot_display(view.cart_slots),
            zeny_display(view.zeny),
        ]
    }
}

/// A labeled weight meter: a `label` / `current/max` header over a fill bar. A
/// zero `max` (server cap not yet received) renders an empty track.
fn meter(label: String, current: u32, max: u32, fill: Color) -> impl Scene {
    let ratio = if max == 0 {
        0.0
    } else {
        (current as f32 / max as f32).clamp(0.0, 1.0)
    };
    let value = format!("{current} / {max}");
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(3), flex_grow: 1.0, flex_basis: px(0), min_width: px(0) }
        ignore_picking()
        Children [
            (
                Node { flex_direction: FlexDirection::Row, justify_content: JustifyContent::SpaceBetween }
                ignore_picking()
                Children [
                    chrome_text(label, 9.5, theme::TEXT_DIM),
                    chrome_text(value, 9.5, theme::TEXT),
                ]
            ),
            (
                Node { height: px(6), border_radius: BorderRadius::all(px(3)) }
                BackgroundColor(theme::FIELD)
                ignore_picking()
                Children [
                    (
                        Node { width: {Val::Percent(ratio * 100.0)}, height: percent(100), border_radius: BorderRadius::all(px(3)) }
                        BackgroundColor(fill)
                        ignore_picking()
                    ),
                ]
            ),
        ]
    }
}

/// The cart slot usage readout: a stacked "SLOTS" label over `used / 100`.
fn slot_display(used: usize) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(1), align_items: AlignItems::FlexEnd }
        ignore_picking()
        Children [
            chrome_text("SLOTS".to_string(), 9.0, theme::TEXT_DIM),
            chrome_text(format!("{used} / {CART_MAX_SLOTS}"), 12.0, theme::TEXT),
        ]
    }
}

/// The footer's zeny display: a coin glyph beside the gold balance.
fn zeny_display(zeny: u32) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(6) }
        ignore_picking()
        Children [
            glyph_icon("coin", 14.0, theme::GOLD),
            (
                Text({format!("{zeny}z")})
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                    font_size: {FontSize::Px(12.0)},
                }
                TextColor(theme::GOLD)
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
            font_size: {FontSize::Px(12.0)},
        }
        TextColor(theme::TEXT_FAINT)
        ignore_picking()
    }
}

/// A plain colored text label with the body font.
fn chrome_text(text: String, size: f32, color: Color) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(size)},
        }
        TextColor(color)
        ignore_picking()
    }
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

fn on_close(_: On<Activate>, mut window: Query<&mut Visibility, With<CartWindowRoot>>) {
    if let Ok(mut visibility) = window.single_mut() {
        *visibility = Visibility::Hidden;
    }
}

/// Drag the single pushcart window by its titlebar; mirrors the inventory/shop
/// windows. Only the titlebar itself moves the window: `Pointer<Drag>` bubbles up
/// from the close button, so a drag targeting it is ignored.
fn on_titlebar_drag(
    drag: On<Pointer<Drag>>,
    titlebars: Query<(), With<CartWindowTitlebar>>,
    mut roots: Query<&mut Node, With<CartWindowRoot>>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;
    use game_engine::domain::inventory::Item;
    use lifthrasir_data::{ItemData, ItemInfo};
    use net_contract::dto::CartItem;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app
    }

    fn bag_item(index: u16, item_id: u32, amount: u16, refine: u8) -> Item {
        Item {
            index,
            item_id,
            item_type: 0,
            amount,
            refine,
            identified: true,
            ..Default::default()
        }
    }

    fn cart_dto(index: u32, amount: u32) -> CartItem {
        CartItem {
            nameid: 501,
            index,
            amount,
            identified: true,
            refine: 0,
            cards: vec![],
            attribute: 0,
            expire_time: 0,
            weight: 10,
        }
    }

    fn potion_db() -> ItemDb {
        let mut data = ItemData::default();
        data.items.insert(
            501,
            ItemInfo {
                identified_name: "Red Potion".to_string(),
                identified_resource: "RED_POTION".to_string(),
                ..Default::default()
            },
        );
        ItemDb::from_item_data(data)
    }

    #[test]
    fn window_spawns_root_and_body() {
        let mut app = test_app();
        app.world_mut()
            .spawn_scene(window())
            .expect("window spawns");
        app.update();

        let roots = app
            .world_mut()
            .query_filtered::<(), With<CartWindowRoot>>()
            .iter(app.world())
            .count();
        assert_eq!(roots, 1, "exactly one CartWindowRoot");

        let bodies = app
            .world_mut()
            .query_filtered::<(), With<CartWindowBody>>()
            .iter(app.world())
            .count();
        assert_eq!(bodies, 1, "exactly one CartWindowBody");
    }

    #[test]
    fn not_mounted_body_carries_a_mount_button() {
        let mut app = test_app();
        let inv = Inventory::default();
        let cart = Cart::default();
        let ui = super::super::CartUi::default();
        app.world_mut()
            .spawn_scene(body(false, &inv, &cart, &ui, None, None))
            .expect("body spawns");
        app.update();

        let mounts: Vec<bool> = app
            .world_mut()
            .query::<&MountToggleButton>()
            .iter(app.world())
            .map(|button| button.mount)
            .collect();
        assert_eq!(mounts, vec![true]);
    }

    #[test]
    fn mount_prompt_renders_rejection_hint() {
        let mut app = test_app();
        let inv = Inventory::default();
        let cart = Cart::default();
        let ui = super::super::CartUi {
            mount_error: Some(CartMountRejection::SkillNotLearned),
            ..Default::default()
        };
        app.world_mut()
            .spawn_scene(body(false, &inv, &cart, &ui, None, None))
            .expect("body spawns");
        app.update();

        let texts: Vec<String> = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect();
        assert!(
            texts.iter().any(|t| t == "You have not learned Pushcart."),
            "rejection hint text present, got {texts:?}"
        );
    }

    #[test]
    fn mount_prompt_without_error_has_no_hint() {
        let mut app = test_app();
        let inv = Inventory::default();
        let cart = Cart::default();
        let ui = super::super::CartUi::default();
        app.world_mut()
            .spawn_scene(body(false, &inv, &cart, &ui, None, None))
            .expect("body spawns");
        app.update();

        let texts: Vec<String> = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect();
        assert!(
            !texts.iter().any(|t| t == "You have not learned Pushcart."),
            "no rejection hint without an error, got {texts:?}"
        );
    }

    #[test]
    fn mounted_body_carries_an_unmount_button() {
        let mut app = test_app();
        let inv = Inventory::default();
        let cart = Cart::default();
        let ui = super::super::CartUi::default();
        let status = CharacterStatus::default();
        app.world_mut()
            .spawn_scene(body(true, &inv, &cart, &ui, Some(&status), None))
            .expect("body spawns");
        app.update();

        let mounts: Vec<bool> = app
            .world_mut()
            .query::<&MountToggleButton>()
            .iter(app.world())
            .map(|button| button.mount)
            .collect();
        assert_eq!(mounts, vec![false]);
    }

    #[test]
    fn bag_cells_projects_unworn_items_with_selection() {
        let mut inv = Inventory::default();
        inv.upsert(bag_item(2, 501, 5, 0));
        inv.upsert(Item {
            index: 3,
            wear_state: 1,
            amount: 1,
            ..Default::default()
        });
        let db = potion_db();
        let cells = bag_cells(&inv, Some(&db), Some((Side::Bag, 2)));

        assert_eq!(cells.len(), 1, "worn equipment excluded from the bag pane");
        assert_eq!(cells[0].index, 2);
        assert_eq!(cells[0].amount, 5);
        assert!(cells[0].selected);
        assert!(cells[0].icon.is_some());
    }

    #[test]
    fn bag_cells_without_db_renders_iconless_cell_no_panic() {
        let mut inv = Inventory::default();
        inv.upsert(bag_item(2, 501, 1, 0));
        let cells = bag_cells(&inv, None, None);
        assert_eq!(cells.len(), 1);
        assert!(cells[0].icon.is_none());
    }

    #[test]
    fn cart_cells_projects_items_with_selection() {
        let mut cart = Cart::default();
        cart.upsert(cart_dto(4, 12));
        let db = potion_db();
        let cells = cart_cells(&cart, Some(&db), Some((Side::Cart, 4)));

        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0].index, 4);
        assert_eq!(cells[0].amount, 12);
        assert!(cells[0].selected);
        assert_eq!(cells[0].side, Side::Cart);
    }

    #[test]
    fn detail_view_resolves_bag_and_cart_selections() {
        let db = potion_db();
        let mut inv = Inventory::default();
        inv.upsert(bag_item(2, 501, 5, 0));
        let mut cart = Cart::default();
        cart.upsert(cart_dto(4, 3));

        let bag = detail_view(Some((Side::Bag, 2)), &inv, &cart, Some(&db)).unwrap();
        assert_eq!(bag.name, "Red Potion");
        assert_eq!(bag.type_label, Some("Usable".to_string()));

        let cart_detail = detail_view(Some((Side::Cart, 4)), &inv, &cart, Some(&db)).unwrap();
        assert_eq!(cart_detail.name, "Red Potion");
        assert_eq!(cart_detail.type_label, None);

        assert!(detail_view(None, &inv, &cart, Some(&db)).is_none());
    }

    #[test]
    fn detail_view_falls_back_to_nameid_without_db() {
        let mut inv = Inventory::default();
        inv.upsert(bag_item(2, 777, 1, 0));
        let cart = Cart::default();
        let view = detail_view(Some((Side::Bag, 2)), &inv, &cart, None).unwrap();
        assert_eq!(view.name, "#777");
    }

    #[test]
    fn footer_view_projects_weights_slots_and_zeny() {
        let mut cart = Cart::default();
        cart.set_weights(120, 8000);
        cart.upsert(cart_dto(4, 1));
        cart.upsert(cart_dto(5, 1));
        let status = CharacterStatus {
            weight: 500,
            max_weight: 12000,
            zeny: 4200,
            ..Default::default()
        };
        let view = footer_view(&cart, &status);

        assert_eq!(view.body_weight, 500);
        assert_eq!(view.body_max, 12000);
        assert_eq!(view.cart_weight, 120);
        assert_eq!(view.cart_max, 8000);
        assert_eq!(view.cart_slots, 2);
        assert_eq!(view.zeny, 4200);
    }
}
