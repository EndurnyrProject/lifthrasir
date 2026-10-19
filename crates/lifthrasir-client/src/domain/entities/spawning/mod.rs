pub mod events;
pub mod plugin;
pub mod remote_equipment;
pub mod systems;

pub use crate::domain::entities::spawning::events::{DespawnEntity, SpawnEntity};
pub use crate::domain::entities::spawning::plugin::EntitySpawningPlugin;
