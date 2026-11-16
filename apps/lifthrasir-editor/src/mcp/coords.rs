//! GAT cells and ground tiles as the MCP tools take them (`[x, y]`, `y` grows
//! north), and every conversion to the tuples the map tools use.

use super::call::AgentError;
use crate::map::{
    layout::MapLayout,
    tools::gat::{GAT_CELL_SIZE, disc},
};
use bevy::prelude::*;
use lifthrasir_runtime::world::LifMapData;
use ro_formats::RoAltitude;
use schemars::JsonSchema;
use serde::Deserialize;

/// The largest circle radius, in cells.
pub const MAX_RADIUS: u32 = 64;

/// A set of GAT cells.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Region {
    /// Exactly these cells.
    Cells(Vec<[u32; 2]>),
    /// Every cell from `min` to `max`, inclusive.
    Rect { min: [u32; 2], max: [u32; 2] },
    /// Cells within `radius` cells of `center`; cells off the map are skipped.
    Circle { center: [u32; 2], radius: u32 },
}

pub fn from_wire([x, y]: [u32; 2]) -> UVec2 {
    UVec2::new(x, y)
}

pub fn to_tuple(cell: UVec2) -> (u32, u32) {
    (cell.x, cell.y)
}

pub fn to_usize(cell: UVec2) -> (usize, usize) {
    (cell.x as usize, cell.y as usize)
}

/// The ground tile holding a GAT cell (one tile is 2 x 2 cells).
pub fn tile_of_cell(cell: UVec2) -> UVec2 {
    cell / 2
}

/// The open map's size in GAT cells.
pub fn map_bounds(world: &World) -> Result<UVec2, AgentError> {
    let root = world
        .get_resource::<MapLayout>()
        .ok_or(AgentError::NoMapOpen)?
        .root;
    let altitude = &world
        .get::<LifMapData>(root)
        .ok_or(AgentError::NoMapOpen)?
        .altitude;
    Ok(UVec2::new(altitude.width, altitude.height))
}

/// `cell`, or `OutOfBounds` naming the map size.
pub fn in_bounds(cell: UVec2, bounds: UVec2) -> Result<UVec2, AgentError> {
    if cell.x < bounds.x && cell.y < bounds.y {
        return Ok(cell);
    }
    Err(AgentError::OutOfBounds(format!(
        "[{}, {}] is outside the {}x{} grid",
        cell.x, cell.y, bounds.x, bounds.y
    )))
}

/// The world point on the walk surface at the centre of `cell`.
pub fn cell_center(altitude: &RoAltitude, cell: UVec2) -> Result<Vec3, AgentError> {
    let cell = in_bounds(cell, UVec2::new(altitude.width, altitude.height))?;
    let x = (cell.x as f32 + 0.5) * GAT_CELL_SIZE;
    let z = -(cell.y as f32 + 0.5) * GAT_CELL_SIZE;
    let y = altitude
        .get_terrain_height_at_position(Vec3::new(x, 0.0, z))
        .ok_or_else(|| {
            AgentError::OutOfBounds(format!("no GAT cell at [{}, {}]", cell.x, cell.y))
        })?;
    Ok(Vec3::new(x, y, z))
}

/// Every cell of `region` on a `bounds`-sized map. `Cells`/`Rect` outside
/// the map are `OutOfBounds`; a `Circle` needs its centre on the map and
/// `radius <= MAX_RADIUS`, and is clipped like the GUI brush.
pub fn region_cells(region: &Region, bounds: UVec2) -> Result<Vec<UVec2>, AgentError> {
    match region {
        Region::Cells(cells) => cells
            .iter()
            .map(|&cell| in_bounds(from_wire(cell), bounds))
            .collect(),
        Region::Rect { min, max } => {
            let (min, max) = (
                in_bounds(from_wire(*min), bounds)?,
                in_bounds(from_wire(*max), bounds)?,
            );
            if min.x > max.x || min.y > max.y {
                return Err(AgentError::Invalid("rect min exceeds max".into()));
            }
            Ok((min.y..=max.y)
                .flat_map(|y| (min.x..=max.x).map(move |x| UVec2::new(x, y)))
                .collect())
        }
        Region::Circle { center, radius } => {
            if *radius > MAX_RADIUS {
                return Err(AgentError::Invalid(format!(
                    "radius {radius} exceeds {MAX_RADIUS}"
                )));
            }
            let center = in_bounds(from_wire(*center), bounds)?;
            Ok(disc(to_tuple(center), *radius, bounds.x, bounds.y)
                .map(|(x, y)| UVec2::new(x, y))
                .collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::tools::gat::gat_cell_at;
    use ro_formats::gat::{GatCell, GatCellType};

    fn altitude(width: u32, height: u32) -> RoAltitude {
        RoAltitude {
            version: "1.2".into(),
            width,
            height,
            cells: vec![
                GatCell {
                    height: [0.0; 4],
                    cell_type: GatCellType::from(0),
                };
                (width * height) as usize
            ],
        }
    }

    #[test]
    fn cell_center_round_trips_through_gat_cell_at() {
        let altitude = altitude(8, 6);
        for cell in [UVec2::new(0, 0), UVec2::new(7, 5), UVec2::new(3, 2)] {
            let at = cell_center(&altitude, cell).unwrap();
            assert_eq!(gat_cell_at(&altitude, at), Some(to_tuple(cell)));
        }
    }

    #[test]
    fn rect_partly_off_map_is_out_of_bounds() {
        let rect = Region::Rect {
            min: [6, 0],
            max: [9, 1],
        };
        let Err(AgentError::OutOfBounds(message)) = region_cells(&rect, UVec2::new(8, 6)) else {
            panic!("expected OutOfBounds");
        };
        assert!(message.contains("8x6"), "{message}");
    }

    #[test]
    fn circle_near_edge_is_clipped() {
        let circle = Region::Circle {
            center: [0, 0],
            radius: 3,
        };
        let cells = region_cells(&circle, UVec2::new(8, 6)).unwrap();
        assert!(!cells.is_empty());
        assert!(cells.contains(&UVec2::new(3, 0)));
        assert!(cells.iter().all(|cell| cell.x < 8 && cell.y < 6));
    }

    #[test]
    fn circle_radius_over_64_is_invalid() {
        let circle = Region::Circle {
            center: [0, 0],
            radius: 65,
        };
        assert!(matches!(
            region_cells(&circle, UVec2::new(8, 6)),
            Err(AgentError::Invalid(_))
        ));
    }

    #[test]
    fn circle_centre_off_map_is_out_of_bounds() {
        let circle = Region::Circle {
            center: [8, 0],
            radius: 1,
        };
        assert!(matches!(
            region_cells(&circle, UVec2::new(8, 6)),
            Err(AgentError::OutOfBounds(_))
        ));
    }

    #[test]
    fn tile_of_cell_halves() {
        assert_eq!(tile_of_cell(UVec2::new(5, 4)), UVec2::new(2, 2));
    }
}
