use bevy::prelude::*;
use std::collections::HashMap;

/// A discrete item lying on the map floor, awaiting pickup.
#[derive(Component, Debug, Clone)]
pub struct FloorItem {
    pub ground_id: u64,
    pub nameid: u32,
    pub amount: u32,
    pub identified: bool,
}

/// Maps a server `ground_id` to its spawned floor-item entity, for dedup on
/// re-enter and targeted despawn on vanish.
#[derive(Resource, Default)]
pub struct FloorItemRegistry(pub HashMap<u64, Entity>);
