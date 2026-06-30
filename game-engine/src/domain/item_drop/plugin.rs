use super::components::FloorItemRegistry;
use super::hover::{floor_item_hover_detection, update_floor_item_bounds, HoveredFloorItem};
use super::spawn::{clear_floor_item_registry, despawn_floor_items, spawn_floor_items};
use crate::core::GameState;
use crate::domain::system_sets::EntityInteractionSystems;
use bevy::prelude::*;

pub struct ItemDropPlugin;

impl Plugin for ItemDropPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FloorItemRegistry>()
            .init_resource::<HoveredFloorItem>()
            .add_systems(
                Update,
                (spawn_floor_items, despawn_floor_items).run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                Update,
                (update_floor_item_bounds, floor_item_hover_detection)
                    .chain()
                    .in_set(EntityInteractionSystems::Hover)
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(OnExit(GameState::InGame), clear_floor_item_registry);
    }
}
