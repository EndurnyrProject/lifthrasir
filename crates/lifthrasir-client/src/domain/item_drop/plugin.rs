use crate::core::GameState;
use crate::domain::item_drop::animation::animate_falling_drops;
use crate::domain::item_drop::components::FloorItemRegistry;
use crate::domain::item_drop::hover::HoveredFloorItem;
use crate::domain::item_drop::pickup::{
    PendingPickups, clear_pending_pickups, handle_pickup_result,
};
use crate::domain::item_drop::pickup_anim::{play_pickup_animation, tick_pickup_anim};
use crate::domain::item_drop::spawn::{
    clear_floor_item_registry, despawn_floor_items, spawn_floor_items,
};
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
