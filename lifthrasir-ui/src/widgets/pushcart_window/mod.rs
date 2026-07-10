//! Pushcart window: the merchant cart's chrome and its mount/unmount entry point.
//!
//! Task 7 builds the shell: `bsn!` chrome (titlebar + swappable body) authored in
//! [`scene`], the [`CartUi`] selection/quantity state, the `PlayerAction::Cart`
//! visibility toggle, and the body's two mount states — a "Mount Pushcart" prompt
//! when the local player has no cart, and a placeholder plus an "Unmount"
//! affordance when it does. The Bag<->Cart panes, move logic, and meters land in
//! Task 8; only [`scene::body`] changes there.
//!
//! Both affordances are server-authoritative: the buttons only emit
//! [`MountCart`]; the cart sprite and the [`Cart`] resource follow the server's
//! `effect_state`/cart events. Unmount is gated locally on an empty cart because
//! aesir rejects a non-empty unmount — a courtesy check, not the source of truth.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_feathers::{FeathersCorePlugin, FeathersPlugins};
use game_engine::core::state::GameState;
use game_engine::domain::cart::Cart;
use game_engine::domain::entities::character::components::UnitState;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::domain::input::{ui_unfocused, PlayerAction};
use leafwing_input_manager::prelude::ActionState;
use net_contract::commands::MountCart;

use crate::theme::feathers_theme::install_norse_theme;

pub mod scene;

/// Which container a selection or move refers to. Task 8 fills in the move logic;
/// Task 7 only needs the type to exist for [`CartUi`].
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

/// Pending selection + quantity for the Bag<->Cart move UI (Task 8). Defaults to
/// no selection and a quantity of one (the stepper's floor).
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct CartUi {
    pub selected: Option<(Side, u16)>,
    pub qty: u16,
}

impl Default for CartUi {
    fn default() -> Self {
        Self {
            selected: None,
            qty: 1,
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
/// state (`effect_state`) or the [`Cart`] resource changes, and once when the
/// body first spawns so the mount prompt shows immediately. A missing body
/// entity (the frame before the chrome's deferred spawn lands) skips silently;
/// the next change retries. A missing local-player [`UnitState`] reads as
/// not-mounted — a player who never received a `UnitStateChange` has no cart.
fn rebuild_body(
    mut commands: Commands,
    cart: Res<Cart>,
    units: Query<Ref<UnitState>, With<LocalPlayer>>,
    bodies: Query<(Entity, Option<&Children>), With<CartWindowBody>>,
    new_bodies: Query<(), Added<CartWindowBody>>,
) {
    let (mounted, unit_changed) = match units.iter().next() {
        Some(unit) => (unit.is_cart_mounted(), unit.is_changed()),
        None => (false, false),
    };
    if !cart.is_changed() && !unit_changed && new_bodies.is_empty() {
        return;
    }
    let Ok((body, children)) = bodies.single() else {
        return;
    };
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    commands
        .spawn_scene(scene::body(mounted, cart.is_empty()))
        .insert(ChildOf(body));
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

/// Reset the selection/quantity state when leaving the game.
fn reset(mut ui: ResMut<CartUi>) {
    *ui = CartUi::default();
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_contract::dto::CartItem;

    fn cart_item(index: u32) -> CartItem {
        CartItem {
            nameid: 501,
            index,
            amount: 1,
            identified: true,
            refine: 0,
            cards: vec![],
            attribute: 0,
            expire_time: 0,
            weight: 10,
        }
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
        });
        app.add_systems(Update, reset);
        app.update();

        assert_eq!(*app.world().resource::<CartUi>(), CartUi::default());
    }
}
