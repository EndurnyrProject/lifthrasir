use bevy::prelude::*;

// Re-export authentication events for convenience
pub use crate::domain::authentication::events::*;

#[derive(Event)]
pub struct MapLoadedEvent {
    pub map_name: String,
}

#[derive(Event)]
pub struct TerrainGeneratedEvent {
    pub width: u32,
    pub height: u32,
}

#[derive(Event)]
pub struct ModelSpawnedEvent {
    pub model_name: String,
    pub entity: Entity,
}
