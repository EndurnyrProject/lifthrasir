pub mod plugin;
pub mod resource;
pub mod systems;

pub use crate::domain::storage::plugin::StoragePlugin;
pub use crate::domain::storage::resource::Storage;
pub use crate::domain::storage::systems::reset_storage;
