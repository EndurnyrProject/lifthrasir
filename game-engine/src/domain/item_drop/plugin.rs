use super::animation::animate_falling_drops;
use super::components::FloorItemRegistry;
use super::hover::{floor_item_hover_detection, update_floor_item_bounds, HoveredFloorItem};
use super::pickup::{
    clear_pending_pickups, handle_floor_item_click, handle_pickup_result, PendingPickups,
};
use super::spawn::{clear_floor_item_registry, despawn_floor_items, spawn_floor_items};
use crate::core::GameState;
use crate::domain::input::systems::{handle_entity_click, handle_terrain_click};
use crate::domain::system_sets::{EntityInteractionSystems, InputSystems};
use bevy::prelude::*;

pub struct ItemDropPlugin;

impl Plugin for ItemDropPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FloorItemRegistry>()
            .init_resource::<HoveredFloorItem>()
            .init_resource::<PendingPickups>()
            .add_systems(
                Update,
                (spawn_floor_items, despawn_floor_items).run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                Update,
                animate_falling_drops.run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                Update,
                (update_floor_item_bounds, floor_item_hover_detection)
                    .chain()
                    .in_set(EntityInteractionSystems::Hover)
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                Update,
                handle_floor_item_click
                    .in_set(InputSystems::Click)
                    .before(handle_terrain_click)
                    .after(handle_entity_click)
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                Update,
                handle_pickup_result.run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                OnExit(GameState::InGame),
                (clear_floor_item_registry, clear_pending_pickups),
            );
    }
}
