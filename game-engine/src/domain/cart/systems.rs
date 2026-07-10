use super::resource::Cart;
use bevy::prelude::*;
use net_contract::events::{CartItemAdded, CartItemRemoved, CartLoaded};

pub fn apply_cart_loaded(mut loaded: MessageReader<CartLoaded>, mut cart: ResMut<Cart>) {
    for dump in loaded.read() {
        cart.begin();
        for item in &dump.items {
            cart.upsert(item.clone());
        }
        cart.finish();
        cart.set_weights(dump.current_weight, dump.max_weight);
    }
}

pub fn apply_cart_item_deltas(
    mut added: MessageReader<CartItemAdded>,
    mut removed: MessageReader<CartItemRemoved>,
    mut cart: ResMut<Cart>,
) {
    for a in added.read() {
        let old_amount = cart.get(a.item.index as u16).map(|i| i.amount).unwrap_or(0);
        let delta = a.item.amount.saturating_sub(old_amount);
        cart.upsert(a.item.clone());
        cart.add_weight(a.item.weight * delta);
    }
    for r in removed.read() {
        let per_unit_weight = cart.get(r.index).map(|item| item.weight).unwrap_or(0);
        cart.remove_amount(r.index, r.amount);
        cart.sub_weight(per_unit_weight * r.amount as u32);
    }
}

pub fn reset_cart(mut cart: ResMut<Cart>) {
    *cart = Cart::default();
}

#[cfg(test)]
mod tests {
    use crate::core::state::GameState;
    use crate::domain::cart::{Cart, CartPlugin};
    use bevy::prelude::*;
    use bevy::state::app::StatesPlugin;
    use net_contract::dto::CartItem;
    use net_contract::events::{CartItemAdded, CartItemRemoved, CartLoaded};

    fn item(index: u32, amount: u32, weight: u32) -> CartItem {
        CartItem {
            nameid: 501,
            index,
            amount,
            identified: true,
            refine: 0,
            cards: vec![],
            attribute: 0,
            expire_time: 0,
            weight,
        }
    }

    fn app_with_cart() -> App {
        let mut app = App::new();
        app.add_message::<CartLoaded>();
        app.add_message::<CartItemAdded>();
        app.add_message::<CartItemRemoved>();
        app.add_plugins(CartPlugin);
        app
    }

    #[test]
    fn cart_loaded_replaces_contents_and_sets_weights() {
        let mut app = app_with_cart();

        app.world_mut().write_message(CartLoaded {
            items: vec![item(2, 5, 3), item(3, 1, 10)],
            current_weight: 25,
            max_weight: 8000,
        });
        app.update();

        let cart = app.world().resource::<Cart>();
        assert_eq!(cart.len(), 2);
        assert!(cart.is_ready());
        assert_eq!(cart.current_weight(), 25);
        assert_eq!(cart.max_weight(), 8000);

        app.world_mut().write_message(CartLoaded {
            items: vec![item(4, 1, 2)],
            current_weight: 2,
            max_weight: 8000,
        });
        app.update();

        let cart = app.world().resource::<Cart>();
        assert_eq!(cart.len(), 1);
        assert_eq!(cart.current_weight(), 2);
    }

    #[test]
    fn cart_item_added_inserts_and_increases_weight() {
        let mut app = app_with_cart();

        app.world_mut().write_message(CartLoaded {
            items: vec![],
            current_weight: 0,
            max_weight: 8000,
        });
        app.update();

        app.world_mut().write_message(CartItemAdded {
            item: item(7, 4, 5),
        });
        app.update();

        let cart = app.world().resource::<Cart>();
        assert_eq!(cart.get(7).unwrap().amount, 4);
        assert_eq!(cart.current_weight(), 20);
    }

    #[test]
    fn cart_item_added_restack_only_counts_the_new_delta() {
        let mut app = app_with_cart();

        app.world_mut().write_message(CartLoaded {
            items: vec![],
            current_weight: 0,
            max_weight: 8000,
        });
        app.update();

        app.world_mut().write_message(CartItemAdded {
            item: item(7, 2, 5),
        });
        app.update();
        assert_eq!(app.world().resource::<Cart>().current_weight(), 10);

        app.world_mut().write_message(CartItemAdded {
            item: item(7, 5, 5),
        });
        app.update();

        let cart = app.world().resource::<Cart>();
        assert_eq!(cart.get(7).unwrap().amount, 5);
        assert_eq!(cart.current_weight(), 25);
    }

    #[test]
    fn cart_item_removed_decrements_stack_and_weight() {
        let mut app = app_with_cart();

        app.world_mut().write_message(CartLoaded {
            items: vec![item(7, 5, 3)],
            current_weight: 15,
            max_weight: 8000,
        });
        app.update();

        app.world_mut().write_message(CartItemRemoved {
            index: 7,
            amount: 2,
            reason: 0,
        });
        app.update();

        let cart = app.world().resource::<Cart>();
        assert_eq!(cart.get(7).unwrap().amount, 3);
        assert_eq!(cart.current_weight(), 9);
    }

    #[test]
    fn cart_item_removed_drops_last_unit_and_zeroes_its_weight() {
        let mut app = app_with_cart();

        app.world_mut().write_message(CartLoaded {
            items: vec![item(7, 1, 3)],
            current_weight: 3,
            max_weight: 8000,
        });
        app.update();

        app.world_mut().write_message(CartItemRemoved {
            index: 7,
            amount: 1,
            reason: 0,
        });
        app.update();

        let cart = app.world().resource::<Cart>();
        assert!(cart.get(7).is_none());
        assert_eq!(cart.current_weight(), 0);
    }

    #[test]
    fn reset_cart_clears_on_zone_change() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.init_state::<GameState>();
        app.add_message::<CartLoaded>();
        app.add_message::<CartItemAdded>();
        app.add_message::<CartItemRemoved>();
        app.add_plugins(CartPlugin);

        app.world_mut().write_message(CartLoaded {
            items: vec![item(2, 5, 3)],
            current_weight: 15,
            max_weight: 8000,
        });
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();
        assert_eq!(app.world().resource::<Cart>().len(), 1);

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::CharacterSelection);
        app.update();

        let cart = app.world().resource::<Cart>();
        assert_eq!(cart.len(), 0);
        assert!(!cart.is_ready());
        assert_eq!(cart.current_weight(), 0);
    }
}
