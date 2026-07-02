use super::animation::animate_falling_drops;
use super::components::FloorItemRegistry;
use super::hover::HoveredFloorItem;
use super::pickup::{clear_pending_pickups, handle_pickup_result, PendingPickups};
use super::pickup_anim::{play_pickup_animation, tick_pickup_anim};
use super::spawn::{clear_floor_item_registry, despawn_floor_items, spawn_floor_items};
use crate::core::GameState;
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
                handle_pickup_result.run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                Update,
                (play_pickup_animation, tick_pickup_anim).run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                OnExit(GameState::InGame),
                (clear_floor_item_registry, clear_pending_pickups),
            );
    }
}
