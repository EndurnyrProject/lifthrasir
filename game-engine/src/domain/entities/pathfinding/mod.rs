mod astar;
mod components;
mod grid;
mod simplify;

use bevy::prelude::*;

pub use astar::find_path;
pub use components::WalkablePath;
pub use grid::PathfindingGrid;
pub use simplify::simplify_path;

#[derive(Resource)]
pub struct CurrentMapPathfindingGrid(pub PathfindingGrid);
