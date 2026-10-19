pub mod item;
pub mod plugin;
pub mod resource;
pub mod systems;
pub mod use_item;

pub use crate::domain::inventory::item::{Item, ItemCategory, ItemOption};
pub use crate::domain::inventory::plugin::InventoryPlugin;
pub use crate::domain::inventory::resource::Inventory;
pub use crate::domain::inventory::systems::{apply_inventory_messages, reset_inventory};
pub use crate::domain::inventory::use_item::UseItemRequested;
