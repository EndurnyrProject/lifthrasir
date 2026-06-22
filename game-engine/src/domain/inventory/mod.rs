pub mod item;
pub mod plugin;
pub mod resource;
pub mod systems;

pub use item::{Item, ItemCategory, ItemOption};
pub use plugin::InventoryPlugin;
pub use resource::Inventory;
pub use systems::{apply_inventory_messages, reset_inventory};
