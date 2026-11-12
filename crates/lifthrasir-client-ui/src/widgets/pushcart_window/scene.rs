//! The Pushcart window scenes: the frame [`build`] spawns once and the body
//! [`rebuild_body`](super::rebuild_body) projects from a [`BodyView`]. Sizes follow
//! the `Pushcart` artboard of the Lifthrasir Widgets design.
//!
//! Every view-model is a pure function of owned data, prepared before the `bsn!`
//! block, so the projection is unit-testable without spawning the tree.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::{ControlOrientation, ScrollArea};
use bevy_feathers::controls::FeathersScrollbar;
use lifthrasir_assets::catalogs::item::registry::ItemDb;
use lifthrasir_client::domain::cart::Cart;
use lifthrasir_client::domain::entities::character::components::status::CharacterStatus;
use lifthrasir_client::domain::inventory::Inventory;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{
    chrome_text, close_window, drag_window, glyph_icon, ignore_picking,
};
use lifthrasir_ui::widgets::primitives::{
    BUTTON_SHADOW, ButtonArt, HoverColors, WINDOW_SHADOW, button_with, close_button, diamond,
    inset, window_title_bar, window_titled,
};
use net_contract::events::CartMountRejection;

use crate::screens::shared::{
    QTY_DIGITS, Ring, flush_button, meter, number_field, overline, row_button, slot_frame,
    small_button, thousands, usable, weight_meter,
};
use crate::widgets::inventory_window::scene::{CellView, slot_marks};
use crate::widgets::inventory_window::{self, LooseItem, cell_view};
use crate::widgets::pushcart_window::{
    CART_MAX_SLOTS, CartAmountField, CartCell, CartMoveLabel, CartPane, CartUi, CartWindowBody,
    CartWindowRoot, CartWindowTitlebar, MountToggleButton, QtyButton, SetUpShopButton, Side,
    move_enabled, move_label, on_cell_click, on_mount_toggle, on_move, on_qty_all, on_qty_step,
    on_set_up_shop,
};

const WINDOW_LEFT: f32 = 200.0;
const WINDOW_TOP: f32 = 40.0;
const WINDOW_WIDTH: f32 = 864.0;
const PANE_WIDTH: f32 = 356.0;
/// Fixed height of each pane's grid, so the window never grows with its content.
const PANE_HEIGHT: f32 = 290.0;
const COLUMNS: usize = 6;
/// Slots a pane draws at least: five whole rows.
const MIN_SLOTS: usize = 30;
const SLOT: f32 = 50.0;
const ICON: f32 = 36.0;
/// Six slots and five gaps must fit the 328px a pane leaves inside its padding.
const SLOT_GAP: f32 = 5.0;
const ROW_GAP: f32 = 7.0;
const SELECTION_SLOT: f32 = 42.0;
const SELECTION_ICON: f32 = 30.0;

/// Spawn the whole window as one scene and parent it under `parent` with a single
/// insert.
pub fn build(commands: &mut Commands, parent: Entity) {
    commands.spawn_scene(window()).insert(ChildOf(parent));
}

fn window() -> impl Scene {
    bsn! {
        CartWindowRoot
        @window_titled()
        Node {
            position_type: PositionType::Absolute,
            left: px(WINDOW_LEFT),
            top: px(WINDOW_TOP),
            width: px(WINDOW_WIDTH + 2.0 * WINDOW_SHADOW),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
        }
        Visibility::Hidden
        Pickable
        Children [
            CartWindowTitlebar
            @window_title_bar("Pushcart", bsn! {
                @close_button()
                on(close_window::<CartWindowRoot>)
            })
            Pickable
            on(drag_window::<CartWindowTitlebar, CartWindowRoot>)
            --
            CartWindowBody
            Node {
                flex_direction: FlexDirection::Column,
                margin: {UiRect::new(px(0), px(0), px(10), px(8))},
            }
            @ignore_picking()
        ]
    }
}

/// The selected item as the bar under the panes shows it.
pub(crate) struct SelectionView {
    pub side: Side,
    pub cell: CellView,
    pub name: String,
    pub meta: String,
    /// How many the selected slot holds.
    pub stack: u16,
}

/// The mounted body: both panes, the selection and the read-outs under them.
pub(crate) struct MountedView {
    pub bag: Vec<CellView>,
    pub cart: Vec<CellView>,
    pub selection: Option<SelectionView>,
    pub qty: u16,
    pub can_move: bool,
    /// Carried weight and its limit.
    pub weight: (u32, u32),
    pub cart_weight: (u32, u32),
    pub zeny: u32,
    /// Scroll offsets of the bag pane and the cart pane.
    pub scroll: [Vec2; 2],
}

/// What the body draws: the mount prompt with the last rejection, or the mounted view.
pub(crate) enum BodyView {
    Prompt(Option<CartMountRejection>),
    Mounted(Box<MountedView>),
}

/// The Bag pane's slots: every item that is not worn (worn equipment lives on the
/// Character window). A missing `ItemDb` entry yields a slot with no icon.
pub(crate) fn bag_cells(
    inventory: &Inventory,
    item_db: Option<&ItemDb>,
    selected: Option<(Side, u16)>,
) -> Vec<CellView> {
    let selected = selected
        .filter(|(side, _)| *side == Side::Bag)
        .map(|(_, index)| index);
    inventory
        .iter()
        .filter(|item| !item.is_equipped())
        .map(|item| cell_view(item, item_db, selected))
        .collect()
}

fn cart_item(item: &net_contract::dto::CartItem) -> LooseItem<'_> {
    LooseItem {
        nameid: item.nameid,
        identified: item.identified,
        refine: item.refine,
        cards: &item.cards,
        amount: item.amount,
    }
}

/// The Cart pane's slots, in cart-index order.
pub(crate) fn cart_cells(
    cart: &Cart,
    item_db: Option<&ItemDb>,
    selected: Option<(Side, u16)>,
) -> Vec<CellView> {
    cart.iter()
        .map(|item| {
            let index = item.index as u16;
            CellView {
                index,
                ..cart_item(item).cell(item_db, selected == Some((Side::Cart, index)))
            }
        })
        .collect()
}

/// The selection bar's view, resolved from whichever container the selection is in.
/// `None` when nothing is selected or the slot has vanished after a server move.
pub(crate) fn selection_view(
    selected: Option<(Side, u16)>,
    inventory: &Inventory,
    cart: &Cart,
    item_db: Option<&ItemDb>,
) -> Option<SelectionView> {
    let (side, index) = selected?;
    let (cell, name, place, stack) = match side {
        Side::Bag => {
            let item = inventory.get(index)?;
            (
                cell_view(item, item_db, None),
                inventory_window::item_name(item, item_db),
                "In your bag",
                item.amount,
            )
        }
        Side::Cart => {
            let slot = cart_item(cart.get(index)?);
            (
                slot.cell(item_db, false),
                slot.name(item_db),
                "In your cart",
                slot.amount.min(u16::MAX.into()) as u16,
            )
        }
    };
    Some(SelectionView {
        side,
        cell,
        name,
        meta: format!("{place} \u{b7} {}", thousands(stack.into())),
        stack,
    })
}

pub(crate) fn mounted_view(
    inventory: &Inventory,
    cart: &Cart,
    cart_ui: &CartUi,
    status: &CharacterStatus,
    item_db: Option<&ItemDb>,
) -> MountedView {
    MountedView {
        bag: bag_cells(inventory, item_db, cart_ui.selected),
        cart: cart_cells(cart, item_db, cart_ui.selected),
        selection: selection_view(cart_ui.selected, inventory, cart, item_db),
        qty: cart_ui.qty,
        can_move: move_enabled(cart_ui.selected, cart_ui.qty, inventory, cart),
        weight: (status.weight, status.max_weight),
        cart_weight: (cart.current_weight(), cart.max_weight()),
        zeny: status.zeny,
        scroll: [Vec2::ZERO; 2],
    }
}

/// The swappable body. Exactly one branch renders.
pub(crate) fn body(view: BodyView) -> impl Scene {
    let content: Vec<Box<dyn Scene>> = vec![match view {
        BodyView::Prompt(error) => Box::new(mount_prompt(error)),
        BodyView::Mounted(view) => Box::new(mounted_body(*view)),
    }];
    bsn! {
        Node { flex_direction: FlexDirection::Column }
        @ignore_picking()
        Children [ {content} ]
    }
}

/// Shown when the local player has no cart: a line of copy, the Mount Pushcart
/// button and, after a rejected mount, why the last attempt failed.
fn mount_prompt(error: Option<CartMountRejection>) -> impl Scene {
    let hint =
        error.map(|reason| chrome_text(mount_error_text(reason), 12.0, ro_dark::STATUS_FULL));
    let hint: Vec<_> = hint.into_iter().collect();
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(12),
            padding: {UiRect::vertical(px(28))},
        }
        @ignore_picking()
        Children [
            @chrome_text("You have no pushcart mounted.".into(), 13.0, ro_dark::BODY)
            --
            @flush_button(ButtonArt::PRIMARY, "Mount Pushcart")
            Node { width: px(160.0 + 2.0 * BUTTON_SHADOW), flex_grow: 0.0, flex_shrink: 0.0 }
            MountToggleButton { mount: true }
            on(on_mount_toggle)
            --
            {hint}
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

/// Shown while the cart is mounted: the panes around the move arrows, the selection
/// bar, then Unmount, the zeny and Set Up Shop.
fn mounted_body(view: MountedView) -> impl Scene {
    let [bag_scroll, cart_scroll] = view.scroll;
    let side = view.selection.as_ref().map(|selection| selection.side);
    let can_move = view.can_move;
    let cart_empty = view.cart.is_empty();
    let (cart_weight, cart_max) = view.cart_weight;
    let full = u64::from(cart_weight) * 100 / u64::from(cart_max.max(1));
    let cart_status = (format!("{full}% full"), ro_dark::MUTED);
    let bag_count = format!("{} / {}", view.bag.len(), inventory_window::CAPACITY);
    let cart_count = format!("{} / {CART_MAX_SLOTS}", view.cart.len());
    let (weight, max_weight) = view.weight;
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(14) }
        @ignore_picking()
        Children [
            Node { column_gap: px(16), align_items: AlignItems::Stretch }
            @ignore_picking()
            Children [
                Node { width: px(PANE_WIDTH), flex_shrink: 0.0, flex_direction: FlexDirection::Column }
                @ignore_picking()
                Children [
                    @pane(Side::Bag, bag_count, view.bag, bag_scroll)
                    --
                    @weight_meter(weight, max_weight)
                ]
                --
                @move_column(side, can_move)
                --
                Node { width: px(PANE_WIDTH), flex_shrink: 0.0, flex_direction: FlexDirection::Column }
                @ignore_picking()
                Children [
                    @pane(Side::Cart, cart_count, view.cart, cart_scroll)
                    --
                    @meter("Cart weight", view.cart_weight, cart_status, ro_dark::BASE_EXP, false)
                ]
            ]
            --
            @selection_bar(view.selection, view.qty, can_move)
            --
            @footer(view.zeny, cart_empty)
        ]
    }
}

/// One pane: its caption over a fixed-height scrolling grid of [`COLUMNS`]-wide rows.
/// Rows are explicit, so no slot can wrap at a fractional UI scale.
fn pane(side: Side, count: String, cells: Vec<CellView>, scroll: Vec2) -> impl Scene {
    let (title, hint) = match side {
        Side::Bag => ("BAG", "Carried on you"),
        Side::Cart => ("CART", "Sellable from here"),
    };
    let total = (cells.len() + 1).next_multiple_of(COLUMNS).max(MIN_SLOTS);
    let mut slots = cells
        .into_iter()
        .map(Some)
        .chain(std::iter::repeat_with(|| None))
        .take(total)
        .map(|cell| slot(side, cell))
        .peekable();
    let mut rows = Vec::new();
    while slots.peek().is_some() {
        rows.push(slot_row(slots.by_ref().take(COLUMNS).collect()));
    }
    bsn! {
        Node { flex_direction: FlexDirection::Column, margin: {UiRect::bottom(px(10))} }
        @ignore_picking()
        Children [
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
                @chrome_text(count, 12.0, ro_dark::STRONG)
                Node { flex_grow: 1.0 }
                --
                @chrome_text(hint.into(), 11.0, ro_dark::MUTED)
            ]
            --
            @inset()
            Node { height: px(PANE_HEIGHT), position_type: PositionType::Relative }
            @ignore_picking()
            Children [
                #grid
                CartPane(side)
                ScrollArea
                ScrollPosition(scroll)
                Pickable
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0), top: px(0), right: px(0), bottom: px(0),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(ROW_GAP),
                    padding: {UiRect::new(px(12), px(14), px(12), px(12))},
                    overflow: {Overflow::scroll_y()},
                }
                Children [ {rows} ]
                --
                @FeathersScrollbar { @target: #grid, @orientation: {ControlOrientation::Vertical} }
                Node {
                    position_type: PositionType::Absolute,
                    right: px(4),
                    top: px(8),
                    bottom: px(8),
                    width: px(6),
                }
            ]
        ]
    }
}

fn slot_row(slots: Vec<Box<dyn Scene>>) -> impl Scene {
    bsn! {
        Node { column_gap: px(SLOT_GAP), flex_shrink: 0.0 }
        @ignore_picking()
        Children [ {slots} ]
    }
}

/// A filled slot selects on click, moves its whole stack on a double-click and
/// inspects on a right-click. An empty one is inert.
fn slot(side: Side, cell: Option<CellView>) -> Box<dyn Scene> {
    let Some(view) = cell else {
        return Box::new(bsn! { @slot_frame(SLOT, Ring::None) @ignore_picking() });
    };
    let (index, ring) = (view.index, view.ring);
    let (stroke, _, _) = ring.style();
    let hover = match ring {
        Ring::None => (ro_dark::SLOT_HOVER, ro_dark::BUTTON_STROKE),
        Ring::Rare | Ring::Selected => (ro_dark::SLOT_HOVER, stroke),
    };
    let colors = HoverColors {
        rest: (ro_dark::SLOT, stroke),
        hover,
    };
    let marks = slot_marks(&view, ICON, true);
    Box::new(bsn! {
        CartCell { side: {side}, index: {index} }
        Hovered
        colors
        @slot_frame(SLOT, ring)
        Pickable
        on(on_cell_click)
        Children [ {marks} ]
    })
}

/// The arrows between the panes. The one pointing away from the selected item's pane
/// is lit and moves it, exactly like the Move button.
fn move_column(selected: Option<Side>, can_move: bool) -> impl Scene {
    let to_cart = can_move && selected == Some(Side::Bag);
    let to_bag = can_move && selected == Some(Side::Cart);
    bsn! {
        Node {
            width: px(72),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: px(16),
        }
        @ignore_picking()
        Children [
            @move_arrow("chevr", "To cart", to_cart)
            --
            @diamond(6.0, ro_dark::DIVIDER, ro_dark::DIVIDER)
            --
            @move_arrow("chevl", "To bag", to_bag)
        ]
    }
}

fn move_arrow(icon: &'static str, label: &'static str, lit: bool) -> impl Scene {
    let art = if lit {
        ButtonArt::PRIMARY
    } else {
        ButtonArt::NORMAL
    };
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(5),
        }
        @ignore_picking()
        Children [
            @button_with(art, glyph_icon(icon, 18.0, ro_dark::BUTTON_LABEL))
            Node {
                width: px(56.0 + 2.0 * BUTTON_SHADOW),
                height: px(44.0 + 2.0 * BUTTON_SHADOW),
                flex_grow: 0.0,
                flex_shrink: 0.0,
                margin: {UiRect::all(px(-BUTTON_SHADOW))},
            }
            @usable(lit, Color::WHITE)
            on(on_move)
            --
            @chrome_text(label.into(), 11.0, ro_dark::MUTED)
        ]
    }
}

/// The bar under the panes: the selected item with its amount and Move, or a hint.
fn selection_bar(selection: Option<SelectionView>, qty: u16, can_move: bool) -> impl Scene {
    let content = match selection {
        Some(view) => selected_item(view, qty, can_move),
        None => vec![Box::new(chrome_text(
            "Select an item \u{b7} Double-click moves the whole stack".into(),
            12.0,
            ro_dark::MUTED,
        )) as Box<dyn Scene>],
    };
    bsn! {
        @inset()
        Node {
            height: px(60),
            padding: {UiRect::new(px(10), px(12), px(0), px(0))},
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: px(12),
        }
        @ignore_picking()
        Children [ {content} ]
    }
}

/// The selected item's slot, name and meta line, the amount controls of a stack, then
/// the Move button.
fn selected_item(view: SelectionView, qty: u16, can_move: bool) -> Vec<Box<dyn Scene>> {
    let ring = view.cell.ring;
    let name_color = if ring == Ring::Rare {
        ro_dark::RARE_RING
    } else {
        ro_dark::STRONG
    };
    let marks = slot_marks(&view.cell, SELECTION_ICON, false);
    let label = move_label(view.side, qty);
    let mut content: Vec<Box<dyn Scene>> = vec![
        Box::new(bsn! {
            @slot_frame(SELECTION_SLOT, ring)
            @ignore_picking()
            Children [ {marks} ]
        }),
        Box::new(bsn! {
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                flex_basis: px(0),
                min_width: px(0),
                overflow: {Overflow::clip()},
            }
            @ignore_picking()
            Children [
                @chrome_text(view.name, 13.0, name_color)
                TextLayout { linebreak: LineBreak::NoWrap }
                --
                @chrome_text(view.meta, 11.0, ro_dark::MUTED)
                TextLayout { linebreak: LineBreak::NoWrap }
            ]
        }),
    ];
    if view.stack > 1 {
        content.push(Box::new(step_button("minus", false)));
        content.push(Box::new(bsn! {
            @number_field(qty.to_string(), QTY_DIGITS)
            CartAmountField
            TextLayout { justify: Justify::Center }
            Node { width: px(60) }
        }));
        content.push(Box::new(step_button("plus", true)));
        content.push(Box::new(bsn! {
            @row_button(ButtonArt::NORMAL, "All", 40.0)
            on(on_qty_all)
        }));
    }
    content.push(Box::new(bsn! {
        @button_with(ButtonArt::PRIMARY, bsn! {
            CartMoveLabel
            @chrome_text(label, 13.0, ro_dark::BUTTON_LABEL)
        })
        Node {
            width: px(160.0 + 2.0 * BUTTON_SHADOW),
            flex_grow: 0.0,
            flex_shrink: 0.0,
            margin: {UiRect::all(px(-BUTTON_SHADOW))},
        }
        @usable(can_move, Color::WHITE)
        on(on_move)
    }));
    content
}

fn step_button(icon: &'static str, inc: bool) -> impl Scene {
    bsn! {
        @small_button(icon, 24.0)
        QtyButton { inc }
        on(on_qty_step)
    }
}

/// Unmount, the zeny balance and the way into the seller flow. Unmount is greyed out
/// until the cart is empty, because the server rejects a loaded unmount.
fn footer(zeny: u32, cart_empty: bool) -> impl Scene {
    let zeny = format!("{} z", thousands(zeny.into()));
    let hint = if cart_empty {
        ""
    } else {
        "Empty the cart before unmounting"
    };
    bsn! {
        Node { align_items: AlignItems::Center, column_gap: px(12) }
        @ignore_picking()
        Children [
            @flush_button(ButtonArt::NORMAL, "Unmount")
            Node { width: px(92.0 + 2.0 * BUTTON_SHADOW), flex_grow: 0.0, flex_shrink: 0.0 }
            @usable(cart_empty, Color::WHITE)
            MountToggleButton { mount: false }
            on(on_mount_toggle)
            --
            Node { align_items: AlignItems::Baseline, column_gap: px(6), flex_shrink: 0.0 }
            @ignore_picking()
            Children [
                @chrome_text("Zeny".into(), 12.0, ro_dark::MUTED)
                --
                @chrome_text(zeny, 12.0, ro_dark::BRONZE_LIGHT)
            ]
            --
            Node { flex_grow: 1.0 }
            @ignore_picking()
            --
            @chrome_text(hint.into(), 11.0, ro_dark::MUTED)
            --
            @button_with(ButtonArt::PRIMARY, bsn! {
                Node { align_items: AlignItems::Center, column_gap: px(8) }
                @ignore_picking()
                Children [
                    @glyph_icon("bag", 14.0, ro_dark::BUTTON_LABEL)
                    --
                    @chrome_text("Set Up Shop".into(), 13.0, ro_dark::BUTTON_LABEL)
                ]
            })
            Node {
                width: px(132.0 + 2.0 * BUTTON_SHADOW),
                flex_grow: 0.0,
                flex_shrink: 0.0,
                margin: {UiRect::all(px(-BUTTON_SHADOW))},
            }
            SetUpShopButton
            on(on_set_up_shop)
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;
    use lifthrasir_client::domain::inventory::Item;
    use lifthrasir_data::{ItemData, ItemInfo};
    use net_contract::dto::CartItem;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app
    }

    fn bag_item(index: u16, item_id: u32, amount: u16) -> Item {
        Item {
            index,
            item_id,
            amount,
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
            options: vec![],
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

    fn mounted(inventory: &Inventory, cart: &Cart, cart_ui: &CartUi) -> BodyView {
        let status = CharacterStatus::default();
        BodyView::Mounted(Box::new(mounted_view(
            inventory, cart, cart_ui, &status, None,
        )))
    }

    fn spawn_body(app: &mut App, view: BodyView) {
        app.world_mut()
            .spawn_scene(body(view))
            .expect("body spawns");
        app.update();
    }

    fn texts(app: &mut App) -> Vec<String> {
        let world = app.world_mut();
        let mut query = world.query::<&Text>();
        query.iter(world).map(|text| text.0.clone()).collect()
    }

    fn mount_buttons(app: &mut App) -> Vec<bool> {
        let world = app.world_mut();
        let mut query = world.query::<&MountToggleButton>();
        query.iter(world).map(|button| button.mount).collect()
    }

    #[test]
    fn window_spawns_one_root_and_one_body() {
        let mut app = test_app();
        app.world_mut()
            .spawn_scene(window())
            .expect("window spawns");
        app.update();

        let world = app.world_mut();
        let mut roots = world.query_filtered::<(), With<CartWindowRoot>>();
        let mut bodies = world.query_filtered::<(), With<CartWindowBody>>();
        assert_eq!(roots.iter(world).count(), 1);
        assert_eq!(bodies.iter(world).count(), 1);
    }

    #[test]
    fn the_prompt_offers_to_mount_and_explains_a_rejection() {
        let mut app = test_app();
        spawn_body(&mut app, BodyView::Prompt(None));
        assert_eq!(mount_buttons(&mut app), vec![true]);
        let rejection = "You have not learned Pushcart.";
        assert!(!texts(&mut app).iter().any(|text| text == rejection));

        let mut app = test_app();
        let error = Some(CartMountRejection::SkillNotLearned);
        spawn_body(&mut app, BodyView::Prompt(error));
        assert!(texts(&mut app).iter().any(|text| text == rejection));
    }

    #[test]
    fn the_mounted_body_draws_whole_rows_of_slots_and_the_unmount_button() {
        let mut app = test_app();
        let mut inventory = Inventory::default();
        inventory.upsert(bag_item(2, 501, 5));
        let mut cart = Cart::default();
        cart.upsert(cart_dto(4, 12));
        spawn_body(&mut app, mounted(&inventory, &cart, &CartUi::default()));

        assert_eq!(mount_buttons(&mut app), vec![false]);
        let world = app.world_mut();
        let mut cells = world.query::<&CartCell>();
        let mut filled: Vec<(Side, u16)> = cells
            .iter(world)
            .map(|cell| (cell.side, cell.index))
            .collect();
        filled.sort_by_key(|(_, index)| *index);
        assert_eq!(filled, vec![(Side::Bag, 2), (Side::Cart, 4)]);
        let mut panes = world.query::<(&CartPane, &Children)>();
        for (_, rows) in panes.iter(world) {
            assert_eq!(rows.len(), MIN_SLOTS / COLUMNS, "five rows of six");
        }
        let shown = texts(&mut app);
        assert!(shown.iter().any(|text| text == "1 / 100"), "{shown:?}");
    }

    #[test]
    fn a_selected_stack_gets_the_amount_controls_and_a_single_item_does_not() {
        let mut inventory = Inventory::default();
        inventory.upsert(bag_item(2, 501, 5));
        inventory.upsert(bag_item(3, 501, 1));
        let cart = Cart::default();
        let amount_fields = |selected| {
            let mut app = test_app();
            let cart_ui = CartUi {
                selected: Some((Side::Bag, selected)),
                qty: 1,
                mount_error: None,
            };
            spawn_body(&mut app, mounted(&inventory, &cart, &cart_ui));
            let world = app.world_mut();
            let mut fields = world.query_filtered::<(), With<CartAmountField>>();
            let mut labels = world.query_filtered::<&Text, With<CartMoveLabel>>();
            let label = labels.single(world).expect("move label").0.clone();
            (fields.iter(world).count(), label)
        };

        assert_eq!(amount_fields(2), (1, "Move 1 to cart".to_string()));
        assert_eq!(amount_fields(3), (0, "Move 1 to cart".to_string()));
    }

    #[test]
    fn bag_cells_leave_out_worn_items_and_ring_the_selection() {
        let mut inv = Inventory::default();
        inv.upsert(bag_item(2, 501, 5));
        inv.upsert(Item {
            index: 3,
            wear_state: 1,
            amount: 1,
            ..Default::default()
        });
        let db = potion_db();

        let cells = bag_cells(&inv, Some(&db), Some((Side::Bag, 2)));
        assert_eq!(cells.len(), 1, "worn equipment excluded from the bag pane");
        assert_eq!((cells[0].index, cells[0].amount), (2, 5));
        assert_eq!(cells[0].ring, Ring::Selected);
        assert!(cells[0].icon.is_some());

        let cells = bag_cells(&inv, None, Some((Side::Cart, 2)));
        assert_eq!(
            cells[0].ring,
            Ring::None,
            "the cart's selection is not the bag's"
        );
        assert!(cells[0].icon.is_none(), "no catalogue, no icon, no panic");
    }

    #[test]
    fn cart_cells_keep_their_cart_index_and_ring_the_selection() {
        let mut cart = Cart::default();
        cart.upsert(cart_dto(4, 12));
        cart.upsert(cart_dto(7, 1));
        let cells = cart_cells(&cart, Some(&potion_db()), Some((Side::Cart, 4)));

        assert_eq!(cells.len(), 2);
        assert_eq!((cells[0].index, cells[0].amount), (4, 12));
        assert_eq!(cells[0].ring, Ring::Selected);
        assert_eq!((cells[1].index, cells[1].ring), (7, Ring::None));
    }

    #[test]
    fn selection_view_resolves_bag_and_cart_selections() {
        let db = potion_db();
        let mut inv = Inventory::default();
        inv.upsert(bag_item(2, 501, 5));
        let mut cart = Cart::default();
        cart.upsert(cart_dto(4, 3));

        let bag = selection_view(Some((Side::Bag, 2)), &inv, &cart, Some(&db)).unwrap();
        assert_eq!(bag.name, "Red Potion");
        assert_eq!(bag.meta, "In your bag \u{b7} 5");
        assert_eq!(bag.stack, 5);

        let in_cart = selection_view(Some((Side::Cart, 4)), &inv, &cart, Some(&db)).unwrap();
        assert_eq!(in_cart.name, "Red Potion");
        assert_eq!(in_cart.meta, "In your cart \u{b7} 3");

        assert!(selection_view(None, &inv, &cart, Some(&db)).is_none());
        assert!(
            selection_view(Some((Side::Cart, 9)), &inv, &cart, Some(&db)).is_none(),
            "a slot that left the cart has nothing to show"
        );
        let unnamed = selection_view(Some((Side::Bag, 2)), &inv, &cart, None).unwrap();
        assert_eq!(unnamed.name, "Item #501");
    }

    #[test]
    fn mounted_view_projects_weights_and_zeny() {
        let mut cart = Cart::default();
        cart.set_weights(120, 8000);
        let status = CharacterStatus {
            weight: 500,
            max_weight: 12000,
            zeny: 4200,
            ..Default::default()
        };
        let view = mounted_view(
            &Inventory::default(),
            &cart,
            &CartUi::default(),
            &status,
            None,
        );

        assert_eq!(view.weight, (500, 12000));
        assert_eq!(view.cart_weight, (120, 8000));
        assert_eq!(view.zeny, 4200);
        assert!(!view.can_move, "nothing is selected");
    }
}
