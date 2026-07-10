use super::resource::Cart;
use super::systems;
use crate::core::state::GameState;
use bevy::prelude::*;

pub struct CartPlugin;

impl Plugin for CartPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Cart>()
            .add_systems(
                Update,
                (
                    systems::apply_cart_loaded,
                    systems::apply_cart_item_deltas.after(systems::apply_cart_loaded),
                ),
            )
            .add_systems(OnEnter(GameState::CharacterSelection), systems::reset_cart);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_contract::events::{CartItemAdded, CartItemRemoved, CartLoaded};

    #[test]
    fn plugin_registers_resource() {
        let mut app = App::new();
        app.add_message::<CartLoaded>();
        app.add_message::<CartItemAdded>();
        app.add_message::<CartItemRemoved>();
        app.add_plugins(CartPlugin);

        assert!(app.world().contains_resource::<Cart>());
    }
}
