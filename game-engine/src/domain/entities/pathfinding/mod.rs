mod astar;
mod components;
mod grid;

use bevy::prelude::*;

pub use astar::find_path;
pub use components::WalkablePath;
pub use grid::PathfindingGrid;

#[derive(Resource)]
pub struct CurrentMapPathfindingGrid(pub PathfindingGrid);
