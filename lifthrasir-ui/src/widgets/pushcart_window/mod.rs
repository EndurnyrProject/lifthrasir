//! Pushcart window: the merchant cart's chrome, its mount/unmount entry point,
//! and the Bag<->Cart move UI.
//!
//! The `bsn!` chrome (titlebar + swappable body) is authored in [`scene`]. The
//! swappable body has two mount states: a "Mount Pushcart" prompt when the local
//! player has no cart, and — when mounted — the two panes (Bag from `Inventory`,
//! Cart from [`Cart`]), a detail strip with a quantity stepper and a Move button,
//! the body/cart weight meters plus slot and zeny footer, and the "Unmount"
//! affordance. [`rebuild_body`] respawns it on any `Inventory`/`Cart`/[`CartUi`]/
//! `CharacterStatus`/mount-state change.
//!
//! Every affordance is server-authoritative. The mount buttons emit [`MountCart`];
//! the Move button emits [`MoveToCart`]/[`MoveFromCart`] but never mutates the
//! panes — they update only when the server's cart/inventory events arrive.
//! Client cap checks (100 slots, cart weight, source stack) only *disable* the
//! Move button; unmount is likewise gated locally on an empty cart because aesir
//! rejects a non-empty unmount — a courtesy check, not the source of truth.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_feathers::{FeathersCorePlugin, FeathersPlugins};
use game_engine::core::state::GameState;
use game_engine::domain::cart::Cart;
use game_engine::domain::entities::character::components::status::CharacterStatus;
use game_engine::domain::entities::character::components::UnitState;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::domain::input::{ui_unfocused, PlayerAction};
use game_engine::domain::inventory::Inventory;
use game_engine::infrastructure::item::ItemDb;
use leafwing_input_manager::prelude::ActionState;
use net_contract::commands::{MountCart, MoveFromCart, MoveToCart};
use net_contract::events::{CartMountRejection, CartMountResult};

use crate::theme::feathers_theme::install_norse_theme;

pub mod scene;

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

/// RO caps the cart at 100 slots; used both to disable the Move button and to
/// label the Cart pane header.
pub(crate) const CART_MAX_SLOTS: usize = 100;

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
        install_norse_theme(app);
        if !app.is_plugin_added::<FeathersCorePlugin>() {
            app.add_plugins(FeathersPlugins);
        }
        app.init_resource::<CartUi>();
        app.add_systems(
            Update,
            toggle_pushcart_window.run_if(in_state(GameState::InGame).and_then(ui_unfocused)),
        );
        app.add_systems(Update, rebuild_body.run_if(in_state(GameState::InGame)));
        app.add_systems(
            Update,
            apply_mount_result.run_if(in_state(GameState::InGame)),
        );
        app.add_systems(OnExit(GameState::InGame), reset);
    }
}

/// Spawn the hidden pushcart window under `parent`. Delegates the BSN chrome to
/// [`scene::build`]; asset paths resolve inside the scene, so no `AssetServer` is
/// needed.
pub fn spawn_pushcart_window(commands: &mut Commands, parent: Entity) {
    scene::build(commands, parent);
}

/// Rebuilds the swappable [`CartWindowBody`] whenever the local player's mount
/// state (`effect_state`), the [`Cart`]/[`Inventory`] resources, the pending
/// [`CartUi`] selection, or the local player's [`CharacterStatus`] changes, and
/// once when the body first spawns so the correct mount state shows immediately.
/// A missing body entity (the frame before the chrome's deferred spawn lands)
/// skips silently; the next change retries. A missing local-player [`UnitState`]
/// reads as not-mounted. When mounted but the local player's `CharacterStatus`
/// is absent the pass is skipped loudly rather than rendering a fabricated
/// footer — `LocalPlayer`/`CharacterStatus` are inserted atomically at character
/// spawn, so their absence here is a real bug, not an expected transient.
#[allow(clippy::too_many_arguments)]
fn rebuild_body(
    mut commands: Commands,
    cart: Res<Cart>,
    inventory: Res<Inventory>,
    cart_ui: Res<CartUi>,
    item_db: Option<Res<ItemDb>>,
    units: Query<Ref<UnitState>, With<LocalPlayer>>,
    statuses: Query<Ref<CharacterStatus>, With<LocalPlayer>>,
    bodies: Query<(Entity, Option<&Children>), With<CartWindowBody>>,
    new_bodies: Query<(), Added<CartWindowBody>>,
) {
    let (mounted, unit_changed) = match units.iter().next() {
        Some(unit) => (unit.is_cart_mounted(), unit.is_changed()),
        None => (false, false),
    };
    let status = statuses.iter().next();
    let status_changed = status.as_ref().map(|s| s.is_changed()).unwrap_or(false);
    if !cart.is_changed()
        && !inventory.is_changed()
        && !cart_ui.is_changed()
        && !unit_changed
        && !status_changed
        && new_bodies.is_empty()
    {
        return;
    }
    let Ok((body, children)) = bodies.single() else {
        return;
    };
    if mounted && status.is_none() {
        warn!("pushcart window rebuild skipped: no LocalPlayer/CharacterStatus while mounted");
        return;
    }
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    commands
        .spawn_scene(scene::body(
            mounted,
            &inventory,
            &cart,
            &cart_ui,
            status.as_deref(),
            item_db.as_deref(),
        ))
        .insert(ChildOf(body));
}

/// Cell click: select the clicked `(side, index)` and reset the quantity stepper
/// to one, mirroring how the shop window resets its pending quantity on select.
fn on_cell_click(click: On<Pointer<Click>>, cells: Query<&CartCell>, mut ui: ResMut<CartUi>) {
    let Ok(cell) = cells.get(click.entity) else {
        return;
    };
    ui.selected = Some((cell.side, cell.index));
    ui.qty = 1;
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

/// Move button: emits [`MoveToCart`] for a Bag selection or [`MoveFromCart`] for
/// a Cart selection. Guarded by [`move_enabled`] so a capped/invalid request is
/// dropped (the button's dimmed look mirrors the same guard). No optimistic
/// mutation — the panes only change when the server's cart/inventory events
/// arrive and `rebuild_body` respawns.
fn on_move(
    _: On<Activate>,
    ui: Res<CartUi>,
    inventory: Res<Inventory>,
    cart: Res<Cart>,
    mut to_cart: MessageWriter<MoveToCart>,
    mut from_cart: MessageWriter<MoveFromCart>,
) {
    if !move_enabled(ui.selected, ui.qty, &inventory, &cart) {
        return;
    }
    match move_intent(ui.selected, ui.qty) {
        Some(MoveIntent::ToCart {
            inventory_index,
            amount,
        }) => {
            to_cart.write(MoveToCart {
                inventory_index,
                amount,
            });
        }
        Some(MoveIntent::FromCart { cart_index, amount }) => {
            from_cart.write(MoveFromCart { cart_index, amount });
        }
        None => {}
    }
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
    mut writer: MessageWriter<MountCart>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
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
    use game_engine::domain::inventory::Item;
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
}
