use crate::utils::constants::CELL_SIZE;
use bevy::{log::info, prelude::Vec3};
use nom::{
    bytes::complete::tag,
    number::complete::{le_f32, le_u32, le_u8},
    IResult,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GatError {
    #[error("Parse error: {0}")]
    ParseError(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GatCellType(u32);

impl GatCellType {
    pub const NONE: Self = Self(0);
    pub const WALKABLE: Self = Self(1 << 0);
    pub const WATER: Self = Self(1 << 1);
    pub const SNIPABLE: Self = Self(1 << 2);

    pub fn is_walkable(&self) -> bool {
        self.0 & Self::WALKABLE.0 != 0
    }

    pub fn is_water(&self) -> bool {
        self.0 & Self::WATER.0 != 0
    }

    pub fn is_snipable(&self) -> bool {
        self.0 & Self::SNIPABLE.0 != 0
    }
}

impl From<u32> for GatCellType {
    fn from(raw_type: u32) -> Self {
        let flags = match raw_type {
            0 => Self::WALKABLE.0 | Self::SNIPABLE.0,
            1 => Self::NONE.0,
            2 => Self::WALKABLE.0 | Self::SNIPABLE.0,
            3 => Self::WALKABLE.0 | Self::SNIPABLE.0 | Self::WATER.0,
            4 => Self::WALKABLE.0 | Self::SNIPABLE.0,
            5 => Self::SNIPABLE.0,
            6 => Self::WALKABLE.0 | Self::SNIPABLE.0,
            _ => {
                info!(
                    "Unknown GAT cell type: {}, treating as non-walkable",
                    raw_type
                );
                Self::NONE.0
            }
        };
        Self(flags)
    }
}

#[derive(Debug, Clone)]
pub struct GatCell {
    pub height: [f32; 4],
    pub cell_type: GatCellType,
}

#[derive(Debug, Clone)]
pub struct RoAltitude {
    pub version: String,
    pub width: u32,
    pub height: u32,
    pub cells: Vec<GatCell>,
}

impl RoAltitude {
    pub fn from_bytes(input: &[u8]) -> Result<Self, GatError> {
        match parse_gat(input) {
            Ok((_, gat)) => Ok(gat),
            Err(e) => Err(GatError::ParseError(e.to_string())),
        }
    }

    pub fn get_cell(&self, x: usize, y: usize) -> Option<&GatCell> {
        if x < self.width as usize && y < self.height as usize {
            self.cells.get(y * self.width as usize + x)
        } else {
            None
        }
    }

    pub fn is_walkable(&self, x: usize, y: usize) -> bool {
        self.get_cell(x, y)
            .map(|cell| cell.cell_type.is_walkable())
            .unwrap_or(false)
    }

    pub fn get_height(&self, x: f32, y: f32) -> f32 {
        let ix = x as usize;
        let iy = y as usize;

        if let Some(cell) = self.get_cell(ix, iy) {
            let fx = x - ix as f32;
            let fy = y - iy as f32;

            // Bilinear interpolation
            let h1 = cell.height[0] * (1.0 - fx) * (1.0 - fy);
            let h2 = cell.height[1] * fx * (1.0 - fy);
            let h3 = cell.height[2] * (1.0 - fx) * fy;
            let h4 = cell.height[3] * fx * fy;

            h1 + h2 + h3 + h4
        } else {
            0.0
        }
    }

    /// Calculates the terrain height at a given world position using bilinear interpolation.
    /// This method should be used for character positioning and gameplay logic.
    /// Returns `None` if the position is outside the map boundaries.
    ///
    /// # Arguments
    /// * `world_pos` - The world position to query (X, Y, Z coordinates)
    ///
    /// # Returns
    /// * `Some(height)` - The interpolated terrain height in world coordinates
    /// * `None` - If the position is outside the terrain bounds
    ///
    pub fn get_terrain_height_at_position(&self, world_pos: Vec3) -> Option<f32> {
        // Convert world position to cell coordinates
        // GAT has 2x the resolution of GND (200×200 vs 100×100), so scale by 2
        let cell_x = (world_pos.x / CELL_SIZE * 2.0).floor() as i32;
        let cell_z = (world_pos.z / CELL_SIZE * 2.0).floor() as i32;

        // Bounds check
        if cell_x < 0 || cell_x >= self.width as i32 || cell_z < 0 || cell_z >= self.height as i32 {
            return None;
        }

        // Get cell at this position (cells are stored row-major: index = z * width + x)
        let cell_index = (cell_z as usize) * (self.width as usize) + (cell_x as usize);
        let cell = self.cells.get(cell_index)?;

        // Calculate fractional position within cell [0.0, 1.0]
        // Account for 2x resolution scaling
        let fx = (world_pos.x / CELL_SIZE * 2.0).fract().abs();
        let fz = (world_pos.z / CELL_SIZE * 2.0).fract().abs();

        // info!("GAT: fx={}, fz={}", fx, fz);

        // Bilinear interpolation based on corner heights
        // GAT height layout: height[0]=bottom-left, height[1]=bottom-right,
        //                    height[2]=top-left, height[3]=top-right
        let v1 = cell.height[0] * (1.0 - fx) * fz;
        let v2 = cell.height[1] * fx * fz;
        let v3 = cell.height[2] * (1.0 - fx) * (1.0 - fz);
        let v4 = cell.height[3] * fx * (1.0 - fz);
        let interpolated_height = v1 + v2 + v3 + v4;

        Some(interpolated_height - 1.5)
    }

    pub fn count_walkable_cells(&self) -> usize {
        self.cells
            .iter()
            .filter(|cell| cell.cell_type.is_walkable())
            .count()
    }

    pub fn count_water_cells(&self) -> usize {
        self.cells
            .iter()
            .filter(|cell| cell.cell_type.is_water())
            .count()
    }
}

fn parse_header(input: &[u8]) -> IResult<&[u8], String> {
    let (input, _) = tag(&b"GRAT"[..])(input)?;
    let (input, major) = le_u8(input)?;
    let (input, minor) = le_u8(input)?;
    Ok((input, format!("{major}.{minor}")))
}

fn parse_cells(input: &[u8], width: u32, height: u32) -> IResult<&[u8], Vec<GatCell>> {
    let count = (width * height) as usize;
    let mut cells = Vec::with_capacity(count);
    let mut current_input = input;

    for _ in 0..count {
        // Parse 4 height values (bottom-left, bottom-right, top-left, top-right)
        let (remaining, h1) = le_f32(current_input)?;
        let (remaining, h2) = le_f32(remaining)?;
        let (remaining, h3) = le_f32(remaining)?;
        let (remaining, h4) = le_f32(remaining)?;

        // Parse cell type flags
        let (remaining, cell_type) = le_u32(remaining)?;

        cells.push(GatCell {
            height: [h1, h2, h3, h4],
            cell_type: GatCellType::from(cell_type),
        });
        current_input = remaining;
    }

    // Log statistics about loaded cells
    if !cells.is_empty() {
        // Calculate min/max heights
        let mut min_height = f32::MAX;
        let mut max_height = f32::MIN;
        for cell in &cells {
            for &h in &cell.height {
                min_height = min_height.min(h);
                max_height = max_height.max(h);
            }
        }

        // Count unique height combinations
        use std::collections::HashSet;
        let unique_combinations: HashSet<_> = cells
            .iter()
            .map(|cell| {
                (
                    (cell.height[0] * 1000.0) as i32,
                    (cell.height[1] * 1000.0) as i32,
                    (cell.height[2] * 1000.0) as i32,
                    (cell.height[3] * 1000.0) as i32,
                )
            })
            .collect();

        // Sample cells from different parts of the map
        let sample_indices = [
            count / 4,     // 25%
            count / 2,     // 50%
            count * 3 / 4, // 75%
            count - 1,     // Last cell
        ];

        info!("GAT Parse Statistics:");
        info!("  Total cells: {}", count);
        info!("  Min height: {}, Max height: {}", min_height, max_height);
        info!(
            "  Unique height combinations: {}",
            unique_combinations.len()
        );
        for &idx in &sample_indices {
            let cell = &cells[idx];
            info!(
                "  Cell[{}]: heights=[{}, {}, {}, {}]",
                idx, cell.height[0], cell.height[1], cell.height[2], cell.height[3]
            );
        }
    }

    Ok((current_input, cells))
}

fn parse_gat(input: &[u8]) -> IResult<&[u8], RoAltitude> {
    let (input, version) = parse_header(input)?;
    let (input, width) = le_u32(input)?;
    let (input, height) = le_u32(input)?;
    let (input, cells) = parse_cells(input, width, height)?;

    info!(
        "Parsed GAT: version={}, width={}, height={}, cells={}",
        version,
        width,
        height,
        cells.len()
    );

    Ok((
        input,
        RoAltitude {
            version,
            width,
            height,
            cells,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_header() {
        let data = b"GRAT\x01\x02";
        let (_, version) = parse_header(data).unwrap();
        assert_eq!(version, "1.2");
    }

    #[test]
    fn test_cell_type_flags() {
        let walkable = GatCellType::WALKABLE;
        assert!(walkable.is_walkable());
        assert!(!walkable.is_water());

        let water = GatCellType::WATER;
        assert!(water.is_water());
        assert!(!water.is_walkable());

        let combined = GatCellType(GatCellType::WALKABLE.0 | GatCellType::WATER.0);
        assert!(combined.is_walkable());
        assert!(combined.is_water());
    }

    #[test]
    fn test_raw_type_conversion() {
        assert!(GatCellType::from(0).is_walkable());
        assert!(!GatCellType::from(1).is_walkable());
        assert!(GatCellType::from(2).is_walkable());
        assert!(GatCellType::from(3).is_walkable());
        assert!(GatCellType::from(3).is_water());
        assert!(GatCellType::from(4).is_walkable());
        assert!(!GatCellType::from(5).is_walkable());
        assert!(GatCellType::from(5).is_snipable());
        assert!(GatCellType::from(6).is_walkable());
    }

    #[test]
    fn test_altitude_access() {
        let gat = RoAltitude {
            version: "1.2".to_string(),
            width: 10,
            height: 10,
            cells: vec![
                GatCell {
                    height: [1.0, 2.0, 3.0, 4.0],
                    cell_type: GatCellType::WALKABLE,
                };
                100
            ],
        };

        assert!(gat.get_cell(0, 0).is_some());
        assert!(gat.get_cell(9, 9).is_some());
        assert!(gat.get_cell(10, 10).is_none());

        assert!(gat.is_walkable(0, 0));
        assert!(!gat.is_walkable(10, 10));

        // Test height interpolation
        let h = gat.get_height(0.5, 0.5);
        assert_eq!(h, 2.5); // Average of 1, 2, 3, 4
    }
}
