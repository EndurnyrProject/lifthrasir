//! Pushcart window: the merchant cart's frame, its mount/unmount entry point, and
//! the Bag<->Cart move UI.
//!
//! The frame (title bar and swappable body) is authored in [`scene`]. The body has
//! two mount states: a "Mount Pushcart" prompt when the local player has no cart,
//! and, when mounted, the two panes (Bag from `Inventory`, Cart from [`Cart`]) around
//! the move arrows, the selection bar with its amount and Move button, both weight
//! meters, the zeny and the Unmount and Set Up Shop buttons. [`rebuild_body`]
//! respawns it when the bag, the cart, the selection, the mount state or the shown
//! weight and zeny change. The amount lives in its field between rebuilds, and
//! [`sync_amount`] keeps it and [`CartUi::qty`] in step.
//!
//! Every affordance is server-authoritative. The mount buttons emit [`MountCart`];
//! the Move button, the arrows and a double-click emit [`MoveToCart`] or
//! [`MoveFromCart`] but never mutate the panes: they update only when the server's
//! cart and inventory events arrive. Client cap checks (100 slots, cart weight,
//! source stack) only disable the Move button; unmount is likewise gated locally on
//! an empty cart because aesir rejects a non-empty unmount. Both are courtesy checks,
//! not the source of truth.

use std::time::Duration;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui_widgets::Activate;
use leafwing_input_manager::prelude::ActionState;
use lifthrasir_assets::catalogs::item::registry::ItemDb;
use lifthrasir_assets::catalogs::random_option::RandomOptionDb;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::cart::Cart;
use lifthrasir_client::domain::entities::character::components::UnitState;
use lifthrasir_client::domain::entities::character::components::status::CharacterStatus;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_client::domain::input::{PlayerAction, ui_unfocused};
use lifthrasir_client::domain::inventory::Inventory;
use lifthrasir_client::domain::vending::{VendingIntent, VendingState};
use net_contract::commands::{MountCart, MoveFromCart, MoveToCart};
use net_contract::events::{CartMountRejection, CartMountResult};

use crate::screens::shared::{light_field_focus, set_field_text, typed_number};
use crate::widgets::info_modal::{InfoContent, ItemAction, ShowInfoModal, view};

pub mod scene;

const DOUBLE_CLICK: Duration = Duration::from_millis(300);

/// Which container a selection or move refers to. A Bag selection moves into the
/// cart ([`MoveToCart`]); a Cart selection moves out of it ([`MoveFromCart`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Side {
    #[default]
    Bag,
    Cart,
}

/// Marks the pushcart-window root so the toggle/close/drag systems can find it.
#[derive(Component, Default, Clone)]
pub struct CartWindowRoot;

/// The swappable body region, rebuilt by [`rebuild_body`] whenever the local
/// player's mount state or the [`Cart`] resource changes.
#[derive(Component, Default, Clone)]
pub struct CartWindowBody;

/// The draggable titlebar; the drag observer only moves the window when the
/// drag's target is the titlebar itself, so dragging from the close button is
/// inert.
#[derive(Component, Default, Clone)]
pub struct CartWindowTitlebar;

/// Marks a mount/unmount button with the request it sends. `mount == false`
/// (Unmount) is additionally gated on an empty cart (see [`mount_command_allowed`]).
#[derive(Component, Clone, Copy, Default)]
pub struct MountToggleButton {
    pub mount: bool,
}

/// A pane cell: which container it belongs to and the slot index within it. A
/// click selects `(side, index)` into [`CartUi`]; the [`Side`] then decides
/// which move command a subsequent Move emits.
#[derive(Component, Clone, Copy, Default)]
pub struct CartCell {
    pub side: Side,
    pub index: u16,
}

/// A quantity-stepper button: `inc == true` steps up, `false` steps down.
#[derive(Component, Clone, Copy, Default)]
pub struct QtyButton {
    pub inc: bool,
}

/// The scrolling grid of one pane; its offset survives a rebuild.
#[derive(Component, Clone, Copy, Default)]
pub struct CartPane(pub Side);

/// The typed amount of the selected stack.
#[derive(Component, Clone, Default)]
pub struct CartAmountField;

/// The Move button's label, which names the amount it moves.
#[derive(Component, Clone, Default)]
pub struct CartMoveLabel;

/// What the Move button says for `qty` of an item selected on `side`.
pub(crate) fn move_label(side: Side, qty: u16) -> String {
    match side {
        Side::Bag => format!("Move {qty} to cart"),
        Side::Cart => format!("Move {qty} to bag"),
    }
}

/// RO caps the cart at 100 slots; used both to disable the Move button and to
/// label the Cart pane header.
pub(crate) const CART_MAX_SLOTS: usize = 100;

/// The pushcart window's "Set Up Shop" affordance: it asks the domain for the
/// seller flow instead of casting the skill from the cart UI.
#[derive(Component, Default, Clone)]
pub struct SetUpShopButton;

fn on_set_up_shop(_: On<Activate>, mut vending: MessageWriter<VendingIntent>) {
    vending.write(VendingIntent::EnterSeller);
}

/// The move a Bag/Cart selection resolves to. Bag selections move *into* the
/// cart; Cart selections move *out of* it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MoveIntent {
    ToCart { inventory_index: u16, amount: u16 },
    FromCart { cart_index: u16, amount: u16 },
}

/// The move command a `(side, index)` selection plus `qty` resolves to, or
/// `None` when nothing is selected. Direction is purely a function of the
/// selected pane — no optimistic state; the server owns the outcome.
pub(crate) fn move_intent(selected: Option<(Side, u16)>, qty: u16) -> Option<MoveIntent> {
    let (side, index) = selected?;
    let amount = qty.max(1);
    Some(match side {
        Side::Bag => MoveIntent::ToCart {
            inventory_index: index,
            amount,
        },
        Side::Cart => MoveIntent::FromCart {
            cart_index: index,
            amount,
        },
    })
}

/// The available stack size of the currently selected slot, resolved from the
/// live `Inventory`/`Cart`, or `None` when nothing is selected or the slot has
/// vanished. Caps both the stepper and the Move button.
pub(crate) fn selected_stack_amount(
    selected: Option<(Side, u16)>,
    inventory: &Inventory,
    cart: &Cart,
) -> Option<u16> {
    let (side, index) = selected?;
    match side {
        Side::Bag => inventory.get(index).map(|item| item.amount),
        Side::Cart => cart
            .get(index)
            .map(|item| item.amount.min(u16::MAX as u32) as u16),
    }
}

/// Whether the cart still has weight headroom. A zero `max_weight` means the
/// server hasn't sent the cap yet, so we don't block on it.
fn cart_has_weight_room(cart: &Cart) -> bool {
    cart.max_weight() == 0 || cart.current_weight() < cart.max_weight()
}

/// Courtesy predicate for the Move button: something is selected, `qty` fits the
/// source stack, and — for a Bag->Cart move — the cart has a free slot and
/// weight headroom. Server stays authoritative; this only greys the button out
/// so an obviously-doomed request isn't sent.
pub(crate) fn move_enabled(
    selected: Option<(Side, u16)>,
    qty: u16,
    inventory: &Inventory,
    cart: &Cart,
) -> bool {
    if qty < 1 {
        return false;
    }
    let Some((side, index)) = selected else {
        return false;
    };
    match side {
        Side::Bag => {
            let Some(item) = inventory.get(index) else {
                return false;
            };
            // A full cart still has room for an item that stacks onto an existing
            // slot; aesir stacks-first, slot-cap-second (cart.ex
            // `find_stackable_index`). Matching on nameid is over-permissive, which
            // is fine — the server is authoritative; over-restrictive is the bug.
            let slot_room =
                cart.len() < CART_MAX_SLOTS || cart.iter().any(|c| c.nameid == item.item_id);
            item.amount >= qty && slot_room && cart_has_weight_room(cart)
        }
        Side::Cart => cart
            .get(index)
            .map(|item| item.amount >= qty as u32)
            .unwrap_or(false),
    }
}

/// Clamps a stepper value into `[1, cap]`, treating a zero cap as one.
pub(crate) fn clamp_qty(qty: u16, cap: u16) -> u16 {
    qty.clamp(1, cap.max(1))
}

/// Pending selection + quantity for the Bag<->Cart move UI, plus the last mount
/// rejection so the mount prompt can explain a failed mount. Defaults to no
/// selection, a quantity of one (the stepper's floor), and no error.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct CartUi {
    pub selected: Option<(Side, u16)>,
    pub qty: u16,
    pub mount_error: Option<CartMountRejection>,
}

impl Default for CartUi {
    fn default() -> Self {
        Self {
            selected: None,
            qty: 1,
            mount_error: None,
        }
    }
}

/// Whether a mount-toggle click may emit its command: mounting is always allowed;
/// unmounting only when the cart is empty, since aesir rejects a non-empty
/// unmount.
fn mount_command_allowed(mount: bool, cart_empty: bool) -> bool {
    mount || cart_empty
}

pub struct PushcartWindowPlugin;

impl Plugin for PushcartWindowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CartUi>();
        app.init_resource::<LastCartClick>();
        app.add_systems(
            Update,
            toggle_pushcart_window.run_if(in_state(GameState::InGame).and_then(ui_unfocused)),
        );
        app.add_systems(
            Update,
            (
                rebuild_body,
                sync_amount,
                light_field_focus::<CartAmountField>,
            )
                .chain()
                .run_if(in_state(GameState::InGame)),
        );
        app.add_systems(
            Update,
            apply_mount_result.run_if(in_state(GameState::InGame)),
        );
        app.add_systems(OnExit(GameState::InGame), reset);
    }
}

/// Spawn the hidden pushcart window under `parent`. Delegates the frame to
/// [`scene::build`]; asset paths resolve inside the scene, so no `AssetServer` is
/// needed.
pub fn spawn_pushcart_window(commands: &mut Commands, parent: Entity) {
    scene::build(commands, parent);
}

/// The bag, the cart and the item catalogues, as the window reads them.
#[derive(SystemParam)]
pub(crate) struct CartItems<'w> {
    inventory: Res<'w, Inventory>,
    cart: Res<'w, Cart>,
    item_db: Option<Res<'w, ItemDb>>,
    option_db: Option<Res<'w, RandomOptionDb>>,
}

/// The local player's mount state and status.
#[derive(SystemParam)]
pub(crate) struct CartOwner<'w, 's> {
    units: Query<'w, 's, Ref<'static, UnitState>, With<LocalPlayer>>,
    statuses: Query<'w, 's, &'static CharacterStatus, With<LocalPlayer>>,
}

/// What [`rebuild_body`] last drew that change detection cannot tell it about.
#[derive(Default, PartialEq)]
pub(crate) struct Shown {
    selected: Option<(Side, u16)>,
    mount_error: Option<CartMountRejection>,
    weight: (u32, u32),
    zeny: u32,
}

/// Rebuilds the swappable [`CartWindowBody`] when the mount state, the [`Cart`] or
/// [`Inventory`], the selection or the last mount rejection change, and once when the
/// body first spawns. `CharacterStatus` changes on every HP tick, so its shown weight
/// and zeny are compared instead of trusting change detection, and [`CartUi::qty`] is
/// left out so typing an amount never respawns its field. Both panes keep their scroll
/// offsets.
///
/// A missing local-player [`UnitState`] reads as not mounted. When mounted but the
/// local player's `CharacterStatus` is absent the pass is skipped loudly rather than
/// drawing a fabricated footer: both are inserted together at character spawn, so
/// their absence here is a real bug, not an expected transient.
fn rebuild_body(
    mut commands: Commands,
    items: CartItems,
    cart_ui: Res<CartUi>,
    owner: CartOwner,
    bodies: Query<(Entity, Ref<CartWindowBody>, Option<&Children>)>,
    panes: Query<(&CartPane, &ScrollPosition)>,
    mut shown: Local<Shown>,
) {
    let Ok((body, mount, children)) = bodies.single() else {
        return;
    };
    let (mounted, unit_changed) = match owner.units.iter().next() {
        Some(unit) => (unit.is_cart_mounted(), unit.is_changed()),
        None => (false, false),
    };
    let status = owner.statuses.iter().next();
    let now = Shown {
        selected: cart_ui.selected,
        mount_error: cart_ui.mount_error,
        weight: status.map_or((0, 0), |status| (status.weight, status.max_weight)),
        zeny: status.map_or(0, |status| status.zeny),
    };
    let dirty = items.cart.is_changed()
        || items.inventory.is_changed()
        || unit_changed
        || mount.is_added()
        || *shown != now;
    if !dirty {
        return;
    }
    let view = match (mounted, status) {
        (false, _) => scene::BodyView::Prompt(cart_ui.mount_error),
        (true, None) => {
            warn!("pushcart window rebuild skipped: no LocalPlayer/CharacterStatus while mounted");
            return;
        }
        (true, Some(status)) => {
            let scroll_of = |side| {
                panes
                    .iter()
                    .find(|(pane, _)| pane.0 == side)
                    .map(|(_, position)| position.0)
                    .unwrap_or_default()
            };
            scene::BodyView::Mounted(Box::new(scene::MountedView {
                scroll: [scroll_of(Side::Bag), scroll_of(Side::Cart)],
                ..scene::mounted_view(
                    &items.inventory,
                    &items.cart,
                    &cart_ui,
                    status,
                    items.item_db.as_deref(),
                )
            }))
        }
    };
    *shown = now;
    for child in children.into_iter().flatten() {
        commands.entity(*child).despawn();
    }
    commands
        .spawn_scene(scene::body(view))
        .insert(ChildOf(body));
}

/// The cart's lock and the two move commands.
#[derive(SystemParam)]
pub(crate) struct CartMoves<'w> {
    vending: Res<'w, VendingState>,
    to_cart: MessageWriter<'w, MoveToCart>,
    from_cart: MessageWriter<'w, MoveFromCart>,
}

impl CartMoves<'_> {
    /// Sends the move of `qty` of the `selected` slot: a Bag selection goes into the
    /// cart and a Cart selection out of it. A shop holding the cart, or a request
    /// [`move_enabled`] rules out, sends nothing. No optimistic mutation: the panes
    /// only change when the server's cart and inventory events arrive.
    fn send(&mut self, selected: Option<(Side, u16)>, qty: u16, items: &CartItems) {
        if self.vending.cart_transfers_locked() {
            return;
        }
        if !move_enabled(selected, qty, &items.inventory, &items.cart) {
            return;
        }
        match move_intent(selected, qty) {
            Some(MoveIntent::ToCart {
                inventory_index,
                amount,
            }) => {
                self.to_cart.write(MoveToCart {
                    inventory_index,
                    amount,
                });
            }
            Some(MoveIntent::FromCart { cart_index, amount }) => {
                self.from_cart.write(MoveFromCart { cart_index, amount });
            }
            None => {}
        }
    }
}

/// Last primary click on a slot, for double-click detection across body rebuilds.
#[derive(Resource, Default)]
pub(crate) struct LastCartClick {
    cell: Option<(Side, u16)>,
    at: Duration,
}

#[derive(SystemParam)]
pub(crate) struct CartClicks<'w> {
    time: Res<'w, Time>,
    last: ResMut<'w, LastCartClick>,
}

impl CartClicks<'_> {
    /// Records a primary click on `cell` and tells whether it completes a double-click.
    fn is_double(&mut self, cell: (Side, u16)) -> bool {
        let now = self.time.elapsed();
        let double =
            self.last.cell == Some(cell) && now.saturating_sub(self.last.at) <= DOUBLE_CLICK;
        *self.last = LastCartClick {
            cell: (!double).then_some(cell),
            at: now,
        };
        double
    }
}

/// A click selects the slot with its whole stack as the amount, and a double-click
/// moves that stack across. A secondary click opens the info modal instead: a Bag
/// slot with its bag action, a Cart slot info-only.
fn on_cell_click(
    click: On<PointerClick>,
    cells: Query<&CartCell>,
    mut ui: ResMut<CartUi>,
    items: CartItems,
    mut clicks: CartClicks,
    mut moves: CartMoves,
    mut info_writer: MessageWriter<ShowInfoModal>,
) {
    let Ok(cell) = cells.get(click.entity) else {
        return;
    };
    if click.button == PointerButton::Secondary {
        let Some(item_db) = items.item_db.as_deref() else {
            warn!("cart: ItemDb not loaded yet, ignoring inspect");
            return;
        };
        let option_db = items.option_db.as_deref();
        let content = match cell.side {
            Side::Bag => items
                .inventory
                .get(cell.index)
                .map(|item| InfoContent::Item {
                    view: view::inventory_item_view(item, item_db, option_db),
                    action: ItemAction::for_bag_item(item),
                }),
            Side::Cart => items.cart.get(cell.index).map(|item| InfoContent::Item {
                view: view::cart_item_view(item, item_db, option_db),
                action: None,
            }),
        };
        if let Some(content) = content {
            info_writer.write(ShowInfoModal { content });
        }
        return;
    }
    let selected = Some((cell.side, cell.index));
    let stack = selected_stack_amount(selected, &items.inventory, &items.cart).unwrap_or(1);
    ui.selected = selected;
    ui.qty = stack;
    if clicks.is_double((cell.side, cell.index)) {
        moves.send(selected, stack, &items);
    }
}

/// Quantity stepper: steps [`CartUi::qty`] within `[1, selected stack amount]`.
/// A step with nothing selected clamps against a cap of one, so `qty` stays 1.
fn on_qty_step(
    activate: On<Activate>,
    buttons: Query<&QtyButton>,
    mut ui: ResMut<CartUi>,
    inventory: Res<Inventory>,
    cart: Res<Cart>,
) {
    let Ok(step) = buttons.get(activate.entity) else {
        return;
    };
    let cap = selected_stack_amount(ui.selected, &inventory, &cart).unwrap_or(1);
    let next = if step.inc {
        ui.qty.saturating_add(1)
    } else {
        ui.qty.saturating_sub(1)
    };
    ui.qty = clamp_qty(next, cap);
}

/// The All button: sets the amount to the whole selected stack.
fn on_qty_all(_: On<Activate>, mut ui: ResMut<CartUi>, inventory: Res<Inventory>, cart: Res<Cart>) {
    ui.qty = selected_stack_amount(ui.selected, &inventory, &cart).unwrap_or(1);
}

/// Keeps the typed amount and [`CartUi::qty`] in step and names the amount on the
/// Move button. A stepper or a new selection rewrites the field; a typed number is
/// clamped to the selected stack and becomes the amount. An emptied field keeps the
/// last amount, so the player can clear it and type another.
fn sync_amount(
    mut ui: ResMut<CartUi>,
    items: CartItems,
    mut fields: Query<&mut EditableText, With<CartAmountField>>,
    mut labels: Query<&mut Text, With<CartMoveLabel>>,
    mut synced: Local<u16>,
) {
    let Some((side, _)) = ui.selected else {
        return;
    };
    if let Ok(mut field) = fields.single_mut() {
        let cap = selected_stack_amount(ui.selected, &items.inventory, &items.cart).unwrap_or(1);
        let typed = typed_number(&field);
        let wanted = typed.map(|typed| clamp_qty(typed.min(u16::MAX.into()) as u16, cap));
        match wanted {
            Some(wanted) if ui.qty == *synced && wanted != ui.qty => ui.qty = wanted,
            _ => {}
        }
        if typed != Some(ui.qty.into()) && (typed.is_some() || ui.qty != *synced) {
            set_field_text(&mut field, &ui.qty.to_string());
        }
        *synced = ui.qty;
    }
    let label = move_label(side, ui.qty);
    for mut text in &mut labels {
        if text.0 != label {
            text.0.clone_from(&label);
        }
    }
}

/// The Move button and the arrows between the panes: move [`CartUi::qty`] of the
/// selected slot.
fn on_move(_: On<Activate>, ui: Res<CartUi>, items: CartItems, mut moves: CartMoves) {
    moves.send(ui.selected, ui.qty, &items);
}

/// Alt+W toggles the pushcart window between hidden and visible.
fn toggle_pushcart_window(
    player: Query<&ActionState<PlayerAction>, With<LocalPlayer>>,
    mut window: Query<&mut Visibility, With<CartWindowRoot>>,
) {
    let Ok(actions) = player.single() else {
        return;
    };
    if !actions.just_pressed(&PlayerAction::Cart) {
        return;
    }
    let Ok(mut visibility) = window.single_mut() else {
        return;
    };
    *visibility = match *visibility {
        Visibility::Hidden => Visibility::Visible,
        _ => Visibility::Hidden,
    };
}

/// Mount/unmount button: emits [`MountCart`] for the button's `mount` flag. The
/// server owns the outcome (design's "no optimistic" rule) — this only sends the
/// request. Unmount is dropped when the cart is non-empty ([`mount_command_allowed`]),
/// mirroring the disabled look the scene bakes in.
fn on_mount_toggle(
    activate: On<Activate>,
    buttons: Query<&MountToggleButton>,
    cart: Res<Cart>,
    vending: Res<VendingState>,
    mut writer: MessageWriter<MountCart>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    if vending.cart_transfers_locked() {
        return;
    }
    if !mount_command_allowed(button.mount, cart.is_empty()) {
        return;
    }
    writer.write(MountCart {
        mount: button.mount,
    });
}

/// Record the server's latest mount outcome into [`CartUi::mount_error`] so the
/// mount prompt can explain a rejection; a successful mount clears it. Marking
/// [`CartUi`] changed here drives `rebuild_body` to re-render the prompt.
fn apply_mount_result(mut results: MessageReader<CartMountResult>, mut ui: ResMut<CartUi>) {
    for result in results.read() {
        ui.mount_error = result.outcome.err();
    }
}

/// Reset the selection/quantity state when leaving the game.
fn reset(mut ui: ResMut<CartUi>) {
    *ui = CartUi::default();
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifthrasir_client::domain::inventory::Item;
    use net_contract::dto::CartItem;

    fn cart_item(index: u32) -> CartItem {
        cart_item_amount(index, 1)
    }

    fn cart_item_amount(index: u32, amount: u32) -> CartItem {
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

    fn bag_item(index: u16, amount: u16) -> Item {
        Item {
            index,
            item_id: 501,
            amount,
            identified: true,
            ..Default::default()
        }
    }

    fn inventory_with(index: u16, amount: u16) -> Inventory {
        let mut inv = Inventory::default();
        inv.upsert(bag_item(index, amount));
        inv
    }

    fn cart_with(index: u32, amount: u32) -> Cart {
        let mut cart = Cart::default();
        cart.set_weights(0, 8000);
        cart.upsert(cart_item_amount(index, amount));
        cart
    }

    #[test]
    fn mount_command_allowed_truth_table() {
        assert!(mount_command_allowed(true, true));
        assert!(mount_command_allowed(true, false));
        assert!(mount_command_allowed(false, true));
        assert!(!mount_command_allowed(false, false));
    }

    fn drain_mount_cmds(app: &mut App) -> Vec<bool> {
        app.world_mut()
            .resource_mut::<Messages<MountCart>>()
            .drain()
            .map(|cmd| cmd.mount)
            .collect()
    }

    fn trigger_toggle(cart: Cart, mount: bool) -> Vec<bool> {
        let mut app = App::new();
        app.init_resource::<VendingState>();
        app.add_message::<MountCart>();
        app.insert_resource(cart);
        let button = app
            .world_mut()
            .spawn(MountToggleButton { mount })
            .observe(on_mount_toggle)
            .id();
        app.world_mut().trigger(Activate { entity: button });
        drain_mount_cmds(&mut app)
    }

    #[test]
    fn mount_button_emits_mount_true() {
        assert_eq!(trigger_toggle(Cart::default(), true), vec![true]);
    }

    #[test]
    fn mount_and_unmount_are_refused_while_a_shop_holds_the_cart() {
        for mount in [true, false] {
            let mut app = App::new();
            app.add_message::<MountCart>()
                .insert_resource(Cart::default())
                .insert_resource(vending_with_setup());
            let button = app
                .world_mut()
                .spawn(MountToggleButton { mount })
                .observe(on_mount_toggle)
                .id();
            app.world_mut().trigger(Activate { entity: button });
            assert!(drain_mount_cmds(&mut app).is_empty());
        }
    }

    #[test]
    fn the_shop_affordance_enters_the_seller_flow() {
        let mut app = App::new();
        app.init_resource::<VendingState>()
            .add_message::<VendingIntent>();
        let button = app
            .world_mut()
            .spawn(SetUpShopButton)
            .observe(on_set_up_shop)
            .id();
        app.world_mut().trigger(Activate { entity: button });
        let written: Vec<VendingIntent> = app
            .world_mut()
            .resource_mut::<Messages<VendingIntent>>()
            .drain()
            .collect();
        assert_eq!(written, vec![VendingIntent::EnterSeller]);
    }

    fn vending_with_setup() -> VendingState {
        let mut state = VendingState::default();
        state.apply(&net_contract::events::VendingIngress {
            generation: net_contract::state::ZoneSessionGeneration::default(),
            payload: net_contract::events::VendingIngressPayload::SetupReady(
                net_contract::dto::VendingSetup {
                    setup_id: "setup-1".into(),
                    max_slots: 12,
                    min_unit_price: 0,
                    max_unit_price: 1_000_000_000,
                    max_title_bytes: 79,
                    sellable_cart_indices: vec![2],
                    sp_spent: 30,
                },
            ),
        });
        state
    }

    #[test]
    fn unmount_button_emits_mount_false_when_cart_empty() {
        assert_eq!(trigger_toggle(Cart::default(), false), vec![false]);
    }

    #[test]
    fn unmount_button_is_dropped_when_cart_non_empty() {
        let mut cart = Cart::default();
        cart.upsert(cart_item(2));
        assert!(trigger_toggle(cart, false).is_empty());
    }

    #[test]
    fn reset_restores_default_selection_and_qty() {
        let mut app = App::new();
        app.init_resource::<VendingState>();
        app.insert_resource(CartUi {
            selected: Some((Side::Cart, 3)),
            qty: 12,
            mount_error: Some(CartMountRejection::AlreadyMounted),
        });
        app.add_systems(Update, reset);
        app.update();

        assert_eq!(*app.world().resource::<CartUi>(), CartUi::default());
    }

    fn run_mount_result(outcome: Result<(), CartMountRejection>) -> Option<CartMountRejection> {
        let mut app = App::new();
        app.init_resource::<VendingState>();
        app.add_message::<CartMountResult>();
        app.init_resource::<CartUi>();
        app.add_systems(Update, apply_mount_result);
        app.world_mut()
            .resource_mut::<Messages<CartMountResult>>()
            .write(CartMountResult { outcome });
        app.update();
        app.world().resource::<CartUi>().mount_error
    }

    #[test]
    fn apply_mount_result_records_rejection() {
        assert_eq!(
            run_mount_result(Err(CartMountRejection::SkillNotLearned)),
            Some(CartMountRejection::SkillNotLearned)
        );
    }

    #[test]
    fn apply_mount_result_clears_error_on_success() {
        let mut app = App::new();
        app.init_resource::<VendingState>();
        app.add_message::<CartMountResult>();
        app.insert_resource(CartUi {
            mount_error: Some(CartMountRejection::AlreadyMounted),
            ..Default::default()
        });
        app.add_systems(Update, apply_mount_result);
        app.world_mut()
            .resource_mut::<Messages<CartMountResult>>()
            .write(CartMountResult { outcome: Ok(()) });
        app.update();

        assert_eq!(app.world().resource::<CartUi>().mount_error, None);
    }

    #[test]
    fn move_intent_resolves_bag_selection_to_move_to_cart() {
        assert_eq!(
            move_intent(Some((Side::Bag, 7)), 3),
            Some(MoveIntent::ToCart {
                inventory_index: 7,
                amount: 3,
            })
        );
    }

    #[test]
    fn move_intent_resolves_cart_selection_to_move_from_cart() {
        assert_eq!(
            move_intent(Some((Side::Cart, 4)), 2),
            Some(MoveIntent::FromCart {
                cart_index: 4,
                amount: 2,
            })
        );
    }

    #[test]
    fn move_intent_is_none_without_selection() {
        assert_eq!(move_intent(None, 5), None);
    }

    #[test]
    fn move_intent_floors_amount_at_one() {
        assert_eq!(
            move_intent(Some((Side::Bag, 7)), 0),
            Some(MoveIntent::ToCart {
                inventory_index: 7,
                amount: 1,
            })
        );
    }

    #[test]
    fn move_enabled_true_for_bag_within_caps() {
        let inv = inventory_with(7, 5);
        let cart = Cart::default();
        assert!(move_enabled(Some((Side::Bag, 7)), 3, &inv, &cart));
    }

    #[test]
    fn move_disabled_when_bag_qty_exceeds_stack() {
        let inv = inventory_with(7, 2);
        let cart = Cart::default();
        assert!(!move_enabled(Some((Side::Bag, 7)), 3, &inv, &cart));
    }

    fn full_cart_of_nameid_501() -> Cart {
        let mut cart = Cart::default();
        for index in 0..CART_MAX_SLOTS as u32 {
            cart.upsert(cart_item_amount(index, 1));
        }
        assert_eq!(cart.len(), CART_MAX_SLOTS);
        cart
    }

    #[test]
    fn move_disabled_when_cart_slots_full_for_a_new_nameid() {
        // Bag item nameid 999 is absent from the full cart, so it would need a
        // new slot — blocked.
        let mut inv = Inventory::default();
        inv.upsert(Item {
            index: 7,
            item_id: 999,
            amount: 5,
            identified: true,
            ..Default::default()
        });
        let cart = full_cart_of_nameid_501();
        assert!(!move_enabled(Some((Side::Bag, 7)), 1, &inv, &cart));
    }

    #[test]
    fn move_enabled_when_cart_full_but_item_stacks_onto_existing_slot() {
        // Bag item nameid 501 already occupies cart slots, so a full cart still
        // has room to stack onto it — Move must NOT be disabled.
        let inv = inventory_with(7, 5);
        let cart = full_cart_of_nameid_501();
        assert!(move_enabled(Some((Side::Bag, 7)), 1, &inv, &cart));
    }

    #[test]
    fn move_disabled_when_cart_at_max_weight() {
        let inv = inventory_with(7, 5);
        let mut cart = Cart::default();
        cart.set_weights(8000, 8000);
        assert!(!move_enabled(Some((Side::Bag, 7)), 1, &inv, &cart));
    }

    #[test]
    fn move_enabled_true_for_cart_within_stack() {
        let inv = Inventory::default();
        let cart = cart_with(2, 5);
        assert!(move_enabled(Some((Side::Cart, 2)), 5, &inv, &cart));
    }

    #[test]
    fn move_disabled_when_cart_qty_exceeds_stack() {
        let inv = Inventory::default();
        let cart = cart_with(2, 5);
        assert!(!move_enabled(Some((Side::Cart, 2)), 6, &inv, &cart));
    }

    #[test]
    fn move_disabled_without_selection() {
        assert!(!move_enabled(
            None,
            1,
            &Inventory::default(),
            &Cart::default()
        ));
    }

    #[test]
    fn clamp_qty_floors_at_one_and_caps() {
        assert_eq!(clamp_qty(0, 10), 1);
        assert_eq!(clamp_qty(50, 5), 5);
        assert_eq!(clamp_qty(3, 10), 3);
        assert_eq!(clamp_qty(5, 0), 1);
    }

    #[test]
    fn selected_stack_amount_reads_bag_and_cart() {
        let inv = inventory_with(7, 4);
        let cart = cart_with(2, 9);
        assert_eq!(
            selected_stack_amount(Some((Side::Bag, 7)), &inv, &cart),
            Some(4)
        );
        assert_eq!(
            selected_stack_amount(Some((Side::Cart, 2)), &inv, &cart),
            Some(9)
        );
        assert_eq!(
            selected_stack_amount(Some((Side::Bag, 99)), &inv, &cart),
            None
        );
        assert_eq!(selected_stack_amount(None, &inv, &cart), None);
    }

    enum Emitted {
        ToCart(u16, u16),
        FromCart(u16, u16),
    }

    fn trigger_move(
        selected: Option<(Side, u16)>,
        qty: u16,
        inv: Inventory,
        cart: Cart,
    ) -> Vec<Emitted> {
        let mut app = App::new();
        app.init_resource::<VendingState>();
        app.add_message::<MoveToCart>();
        app.add_message::<MoveFromCart>();
        app.insert_resource(CartUi {
            selected,
            qty,
            mount_error: None,
        });
        app.insert_resource(inv);
        app.insert_resource(cart);
        let button = app.world_mut().spawn_empty().observe(on_move).id();
        app.world_mut().trigger(Activate { entity: button });

        let mut out: Vec<Emitted> = app
            .world_mut()
            .resource_mut::<Messages<MoveToCart>>()
            .drain()
            .map(|cmd| Emitted::ToCart(cmd.inventory_index, cmd.amount))
            .collect();
        out.extend(
            app.world_mut()
                .resource_mut::<Messages<MoveFromCart>>()
                .drain()
                .map(|cmd| Emitted::FromCart(cmd.cart_index, cmd.amount)),
        );
        out
    }

    #[test]
    fn move_button_emits_move_to_cart_for_bag_selection() {
        let emitted = trigger_move(
            Some((Side::Bag, 7)),
            3,
            inventory_with(7, 5),
            Cart::default(),
        );
        assert!(matches!(emitted.as_slice(), [Emitted::ToCart(7, 3)]));
    }

    #[test]
    fn move_button_emits_move_from_cart_for_cart_selection() {
        let emitted = trigger_move(
            Some((Side::Cart, 2)),
            4,
            Inventory::default(),
            cart_with(2, 9),
        );
        assert!(matches!(emitted.as_slice(), [Emitted::FromCart(2, 4)]));
    }

    #[test]
    fn move_button_emits_nothing_when_disabled_by_cap() {
        let emitted = trigger_move(
            Some((Side::Bag, 7)),
            9,
            inventory_with(7, 5),
            Cart::default(),
        );
        assert!(emitted.is_empty());
    }

    fn click_event(target: Entity, window: Entity, button: PointerButton) -> PointerClick {
        use bevy::camera::NormalizedRenderTarget;
        use bevy::picking::backend::HitData;
        use bevy::picking::pointer::{Location, PointerId};
        use bevy::window::WindowRef;

        PointerClick {
            entity: target,
            pointer: Pointer::new(
                PointerId::Mouse,
                Location {
                    target: NormalizedRenderTarget::Window(
                        WindowRef::Primary.normalize(Some(window)).unwrap(),
                    ),
                    position: Vec2::ZERO,
                },
            ),
            button,
            hit: HitData::new(target, 0.0, None, None),
            duration: std::time::Duration::ZERO,
            count: 1,
        }
    }

    fn cell_click_app() -> App {
        let mut app = App::new();
        app.init_resource::<VendingState>();
        app.add_message::<ShowInfoModal>();
        app.add_message::<MoveToCart>();
        app.add_message::<MoveFromCart>();
        app.init_resource::<Time>();
        app.init_resource::<LastCartClick>();
        app.init_resource::<CartUi>();
        app.init_resource::<Inventory>();
        app.init_resource::<Cart>();
        app.init_resource::<ItemDb>();
        app
    }

    #[test]
    fn a_click_selects_the_whole_stack_and_a_double_click_moves_it() {
        let mut app = cell_click_app();
        app.world_mut()
            .resource_mut::<Cart>()
            .upsert(cart_item_amount(2, 9));
        let window = app.world_mut().spawn_empty().id();
        let cell = app
            .world_mut()
            .spawn(CartCell {
                side: Side::Cart,
                index: 2,
            })
            .observe(on_cell_click)
            .id();
        let moved = |app: &mut App| -> Vec<(u16, u16)> {
            app.world_mut()
                .resource_mut::<Messages<MoveFromCart>>()
                .drain()
                .map(|cmd| (cmd.cart_index, cmd.amount))
                .collect()
        };

        app.world_mut()
            .trigger(click_event(cell, window, PointerButton::Primary));
        assert_eq!(app.world().resource::<CartUi>().qty, 9);
        assert!(moved(&mut app).is_empty(), "one click only selects");

        app.world_mut()
            .trigger(click_event(cell, window, PointerButton::Primary));
        assert_eq!(moved(&mut app), vec![(2, 9)]);

        app.world_mut()
            .trigger(click_event(cell, window, PointerButton::Primary));
        assert!(moved(&mut app).is_empty(), "a third click starts over");
    }

    fn amount_app(stack: u32, qty: u16, typed: &str) -> (App, Entity) {
        let mut app = App::new();
        app.insert_resource(CartUi {
            selected: Some((Side::Cart, 2)),
            qty,
            mount_error: None,
        });
        app.init_resource::<Inventory>();
        app.insert_resource(cart_with(2, stack));
        app.add_systems(Update, sync_amount);
        let field = app
            .world_mut()
            .spawn((CartAmountField, EditableText::new(typed)))
            .id();
        app.world_mut()
            .spawn((CartMoveLabel, Text::new(String::new())));
        app.update();
        (app, field)
    }

    fn amount_state(app: &mut App, field: Entity) -> (u16, String, String) {
        let typed = app.world().get::<EditableText>(field).unwrap();
        let typed = typed.value().to_string();
        let qty = app.world().resource::<CartUi>().qty;
        let world = app.world_mut();
        let mut labels = world.query_filtered::<&Text, With<CartMoveLabel>>();
        let label = labels.single(world).expect("move label").0.clone();
        (qty, typed, label)
    }

    #[test]
    fn a_typed_amount_is_clamped_to_the_stack_and_names_the_move() {
        let (mut app, field) = amount_app(9, 9, "9");
        assert_eq!(
            amount_state(&mut app, field),
            (9, "9".to_string(), "Move 9 to bag".to_string())
        );

        *app.world_mut().get_mut::<EditableText>(field).unwrap() = EditableText::new("4");
        app.update();
        assert_eq!(
            amount_state(&mut app, field),
            (4, "4".to_string(), "Move 4 to bag".to_string())
        );

        *app.world_mut().get_mut::<EditableText>(field).unwrap() = EditableText::new("500");
        app.update();
        assert_eq!(amount_state(&mut app, field).0, 9, "no more than the stack");

        // An emptied field keeps the amount, so another number can be typed.
        *app.world_mut().get_mut::<EditableText>(field).unwrap() = EditableText::new("");
        app.update();
        assert_eq!(amount_state(&mut app, field).0, 9);
    }

    #[test]
    fn the_stepper_and_all_rewrite_the_typed_amount() {
        let (mut app, field) = amount_app(9, 9, "9");
        let down = app
            .world_mut()
            .spawn(QtyButton { inc: false })
            .observe(on_qty_step)
            .id();
        let all = app.world_mut().spawn_empty().observe(on_qty_all).id();

        app.world_mut().trigger(Activate { entity: down });
        app.update();
        assert_eq!(amount_state(&mut app, field).0, 8);

        app.world_mut().trigger(Activate { entity: all });
        app.update();
        assert_eq!(
            amount_state(&mut app, field),
            (9, "9".to_string(), "Move 9 to bag".to_string())
        );
    }

    #[test]
    fn secondary_click_on_a_filled_bag_cell_opens_the_info_modal_without_selecting() {
        let mut app = cell_click_app();
        app.world_mut()
            .resource_mut::<Inventory>()
            .upsert(bag_item(7, 5));
        let window = app.world_mut().spawn_empty().id();
        let cell = app
            .world_mut()
            .spawn(CartCell {
                side: Side::Bag,
                index: 7,
            })
            .observe(on_cell_click)
            .id();

        app.world_mut()
            .trigger(click_event(cell, window, PointerButton::Secondary));

        let messages = app.world().resource::<Messages<ShowInfoModal>>();
        let mut reader = messages.get_cursor();
        let contents: Vec<_> = reader.read(messages).map(|m| m.content.clone()).collect();
        assert_eq!(contents.len(), 1);
        assert!(matches!(
            &contents[0],
            InfoContent::Item { view, .. } if view.favorite.is_some()
        ));
        assert_eq!(app.world().resource::<CartUi>().selected, None);
    }

    #[test]
    fn secondary_click_on_a_filled_cart_cell_opens_the_info_modal() {
        let mut app = cell_click_app();
        app.world_mut().resource_mut::<Cart>().upsert(cart_item(2));
        let window = app.world_mut().spawn_empty().id();
        let cell = app
            .world_mut()
            .spawn(CartCell {
                side: Side::Cart,
                index: 2,
            })
            .observe(on_cell_click)
            .id();

        app.world_mut()
            .trigger(click_event(cell, window, PointerButton::Secondary));

        let messages = app.world().resource::<Messages<ShowInfoModal>>();
        let mut reader = messages.get_cursor();
        let contents: Vec<_> = reader.read(messages).map(|m| m.content.clone()).collect();
        assert_eq!(contents.len(), 1);
        assert!(matches!(
            &contents[0],
            InfoContent::Item { view, action: None } if view.favorite.is_none()
        ));
        assert_eq!(app.world().resource::<CartUi>().selected, None);
    }

    #[test]
    fn secondary_click_on_an_empty_cell_writes_nothing() {
        let mut app = cell_click_app();
        let window = app.world_mut().spawn_empty().id();
        let cell = app
            .world_mut()
            .spawn(CartCell {
                side: Side::Cart,
                index: 2,
            })
            .observe(on_cell_click)
            .id();

        app.world_mut()
            .trigger(click_event(cell, window, PointerButton::Secondary));

        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<ShowInfoModal>>()
                .drain()
                .count(),
            0
        );
        assert_eq!(app.world().resource::<CartUi>().selected, None);
    }

    #[test]
    fn primary_click_on_a_cell_still_selects_and_does_not_open_the_modal() {
        let mut app = cell_click_app();
        app.world_mut().resource_mut::<Cart>().upsert(cart_item(2));
        let window = app.world_mut().spawn_empty().id();
        let cell = app
            .world_mut()
            .spawn(CartCell {
                side: Side::Cart,
                index: 2,
            })
            .observe(on_cell_click)
            .id();

        app.world_mut()
            .trigger(click_event(cell, window, PointerButton::Primary));

        assert_eq!(
            app.world().resource::<CartUi>().selected,
            Some((Side::Cart, 2))
        );
        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<ShowInfoModal>>()
                .drain()
                .count(),
            0
        );
    }
}
