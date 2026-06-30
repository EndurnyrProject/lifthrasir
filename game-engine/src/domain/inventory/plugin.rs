use super::resource::Inventory;
use super::systems;
use crate::core::state::GameState;
use bevy::prelude::*;

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Inventory>()
            .add_systems(
                Update,
                (
                    systems::apply_inventory_messages,
                    systems::apply_item_deltas.after(systems::apply_inventory_messages),
                ),
            )
            .add_systems(
                OnEnter(GameState::CharacterSelection),
                systems::reset_inventory,
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_contract::events::InventoryReceived;

    #[test]
    fn plugin_registers_resource() {
        let mut app = App::new();
        app.add_message::<InventoryReceived>();
        app.add_plugins(InventoryPlugin);

        assert!(app.world().contains_resource::<Inventory>());
    }
}
