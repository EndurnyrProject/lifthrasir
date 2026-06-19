use super::events::{InventoryDumpCompleted, InventoryDumpStarted, InventoryItemsReceived};
use super::resource::Inventory;
use bevy::prelude::*;

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Inventory>()
            .add_message::<InventoryDumpStarted>()
            .add_message::<InventoryItemsReceived>()
            .add_message::<InventoryDumpCompleted>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_registers_messages_and_resource() {
        let mut app = App::new();
        app.add_plugins(InventoryPlugin);

        assert!(app.world().contains_resource::<Inventory>());
        assert!(app
            .world()
            .contains_resource::<Messages<InventoryItemsReceived>>());
        assert!(app
            .world()
            .contains_resource::<Messages<InventoryDumpStarted>>());
        assert!(app
            .world()
            .contains_resource::<Messages<InventoryDumpCompleted>>());
    }
}
