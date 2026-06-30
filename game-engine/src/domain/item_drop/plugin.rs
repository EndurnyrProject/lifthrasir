use super::components::FloorItemRegistry;
use super::spawn::{clear_floor_item_registry, despawn_floor_items, spawn_floor_items};
use crate::core::GameState;
use bevy::prelude::*;

pub struct ItemDropPlugin;

impl Plugin for ItemDropPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FloorItemRegistry>()
            .add_systems(
                Update,
                (spawn_floor_items, despawn_floor_items).run_if(in_state(GameState::InGame)),
            )
            .add_systems(OnExit(GameState::InGame), clear_floor_item_registry);
    }
}
