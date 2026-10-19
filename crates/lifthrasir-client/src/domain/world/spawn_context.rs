use bevy::prelude::*;

/// Resource that holds the context for spawning a map and positioning the player/camera
/// This is populated when entering the game world from character selection
#[derive(Resource, Debug, Reflect)]
#[reflect(Resource, Debug)]
pub struct MapSpawnContext {
    /// Name of the map to load (e.g., "prontera", "aldebaran")
    pub map_name: String,
    /// X coordinate in cells where the player spawns
    pub spawn_x: u16,
    /// Y coordinate in cells where the player spawns
    pub spawn_y: u16,
    /// Character ID for the player
    pub character_id: u32,
}

impl MapSpawnContext {
    pub fn new(map_name: String, spawn_x: u16, spawn_y: u16, character_id: u32) -> Self {
        Self {
            map_name,
            spawn_x,
            spawn_y,
            character_id,
        }
    }
}
