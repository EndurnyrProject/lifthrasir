use bevy::prelude::*;

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
