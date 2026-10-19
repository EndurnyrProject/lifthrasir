//! Terrain geometry and path queries over caller-selected GAT data.

mod astar;
mod grid;
mod raycast;
mod simplify;

pub use crate::rendering::coordinates::*;
pub use crate::rendering::{Direction, constants, coordinates};
pub use astar::find_path;
pub use grid::PathfindingGrid;
pub use raycast::{TerrainHit, raycast_terrain};
pub use simplify::simplify_path;
