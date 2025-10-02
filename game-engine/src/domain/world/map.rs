use bevy::prelude::*;

#[derive(Component)]
pub struct MapData {
    pub name: String,
    pub width: u32,
    pub height: u32,
}
