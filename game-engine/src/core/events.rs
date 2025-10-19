use bevy::prelude::*;

#[derive(Message)]
pub struct MapLoadedEvent {
    pub map_name: String,
}

#[derive(Message)]
pub struct TerrainGeneratedEvent {
    pub width: u32,
    pub height: u32,
}

#[derive(Message)]
pub struct ModelSpawnedEvent {
    pub model_name: String,
    pub entity: Entity,
}
