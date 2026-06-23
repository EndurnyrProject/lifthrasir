pub mod item;
pub mod plugin;
pub mod resource;
pub mod systems;
pub mod use_item;

pub use item::{Item, ItemCategory, ItemOption};
pub use plugin::InventoryPlugin;
pub use resource::Inventory;
pub use systems::{apply_inventory_messages, reset_inventory};
pub use use_item::UseItemRequested;
