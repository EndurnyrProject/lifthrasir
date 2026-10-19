mod components;

use bevy::prelude::*;

pub use crate::domain::entities::pathfinding::components::WalkablePath;
use lifthrasir_runtime::world::terrain::PathfindingGrid;

#[derive(Resource)]
pub struct CurrentMapPathfindingGrid(pub PathfindingGrid);
