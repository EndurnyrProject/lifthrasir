use crate::utils::{constants::CELL_SIZE, string_utils::parse_korean_string};
use bevy::{
    log::{error, info},
    prelude::Vec3,
};
use nom::{
    bytes::complete::{tag, take},
    number::complete::{le_f32, le_i32, le_u16, le_u32, le_u8},
    IResult, Parser,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GndError {
    #[error("Parse error: {0}")]
    ParseError(String),
}

#[derive(Debug, Clone)]
pub struct GndTile {
    pub u1: f32,
    pub u2: f32,
    pub u3: f32,
    pub u4: f32,
    pub v1: f32,
    pub v2: f32,
    pub v3: f32,
    pub v4: f32,
    pub texture: u16,
    pub color: [u8; 4],
}

#[derive(Debug, Clone)]
pub struct GndSurface {
    pub height: [f32; 4],
    pub tile_up: i32,
    pub tile_front: i32,
    pub tile_right: i32,
}

#[derive(Debug, Clone)]
pub struct RoGround {
    pub version: String,
    pub width: u32,
    pub height: u32,
    pub textures: Vec<String>,
    pub texture_indexes: Vec<usize>,
    pub tiles: Vec<GndTile>,
    pub surfaces: Vec<GndSurface>,
}

impl RoGround {
    pub fn from_bytes(input: &[u8]) -> Result<Self, GndError> {
        match parse_gnd(input) {
            Ok((_, gnd)) => {
                info!(
                    "Parsed GND: version={}, width={}, height={}, surfaces={}",
                    gnd.version,
                    gnd.width,
                    gnd.height,
                    gnd.surfaces.len()
                );
                Ok(gnd)
            }
            Err(e) => {
                error!("GND parse error: {:?}", e);
                Err(GndError::ParseError(e.to_string()))
            }
        }
    }

    /// Calculates the terrain height at a given world position using bilinear interpolation.
    /// Returns `None` if the position is outside the map boundaries.
    ///
    /// # Arguments
    /// * `world_pos` - The world position to query (X, Y, Z coordinates)
    ///
    /// # Returns
    /// * `Some(height)` - The interpolated terrain height in world coordinates
    /// * `None` - If the position is outside the terrain bounds
    pub fn get_terrain_height_at_position(&self, world_pos: Vec3) -> Option<f32> {
        // Convert world position to cell coordinates using floor for correct negative handling
        let cell_x = (world_pos.x / CELL_SIZE).floor() as i32;
        let cell_z = (world_pos.z / CELL_SIZE).floor() as i32;

        // Bounds check
        if cell_x < 0 || cell_x >= self.width as i32 || cell_z < 0 || cell_z >= self.height as i32 {
            return None;
        }

        // Get surface at this cell (surfaces are stored row-major: index = z * width + x)
        let surface_index = (cell_z as usize) * (self.width as usize) + (cell_x as usize);
        let surface = self.surfaces.get(surface_index)?;

        // Calculate fractional position within cell [0.0, 1.0]
        let fx = (world_pos.x / CELL_SIZE).fract().abs();
        let fz = (world_pos.z / CELL_SIZE).fract().abs();

        // Bilinear interpolation based on corner heights
        // height[0]=SW, height[1]=SE, height[2]=NW, height[3]=NE
        let h_sw = surface.height[0];
        let h_se = surface.height[1];
        let h_nw = surface.height[2];
        let h_ne = surface.height[3];

        let height_south = h_sw * (1.0 - fx) + h_se * fx;
        let height_north = h_nw * (1.0 - fx) + h_ne * fx;
        let interpolated_height = height_south * (1.0 - fz) + height_north * fz;

        Some(interpolated_height)
    }
}

fn parse_header(input: &[u8]) -> IResult<&[u8], String> {
    let (input, _) = tag(&b"GRGN"[..])(input)?;
    let (input, major) = le_u8(input)?;
    let (input, minor) = le_u8(input)?;
    Ok((input, format!("{major}.{minor}")))
}

fn parse_textures(input: &[u8]) -> IResult<&[u8], (Vec<String>, Vec<usize>)> {
    let (input, count) = le_u32(input)?;
    let (input, length) = le_u32(input)?;

    let mut indexes = Vec::with_capacity(count as usize);
    let mut unique_textures = Vec::new();
    let mut current_input = input;

    for _ in 0..count {
        let (remaining, texture) = parse_korean_string(current_input, length as usize)?;
        let pos = if let Some(idx) = unique_textures.iter().position(|t| t == &texture) {
            idx
        } else {
            unique_textures.push(texture.clone());
            unique_textures.len() - 1
        };

        indexes.push(pos);
        current_input = remaining;
    }

    Ok((current_input, (unique_textures, indexes)))
}

fn parse_lightmap(input: &[u8]) -> IResult<&[u8], &str> {
    let (input, count) = le_u32(input)?;
    let (input, per_cell_x) = le_i32(input)?;
    let (input, per_cell_y) = le_i32(input)?;
    let (input, size_cell) = le_i32(input)?;
    let per_cell = (per_cell_x * per_cell_y * size_cell) as u32;

    let data_size = (count * per_cell * 4) as usize;

    let (input, _) = take(data_size)(input)?;

    Ok((input, "meh"))
}

fn parse_tiles<'a>(input: &'a [u8], count: u32, version: &str) -> IResult<&'a [u8], Vec<GndTile>> {
    let mut tiles = Vec::with_capacity(count as usize);
    let mut current_input = input;

    for _ in 0..count {
        let (remaining, (u1, u2, u3, u4)) =
            (le_f32, le_f32, le_f32, le_f32).parse(current_input)?;
        let (remaining, (v1, v2, v3, v4)) = (le_f32, le_f32, le_f32, le_f32).parse(remaining)?;
        let (remaining, texture) = le_u16(remaining)?;
        let (remaining, _) = le_u16(remaining)?; // Light, we have our own better lightmaps

        let (remaining, color) = if version >= "1.7" {
            let (remaining, a) = le_u8(remaining)?;
            let (remaining, r_val) = le_u8(remaining)?;
            let (remaining, g_val) = le_u8(remaining)?;
            let (remaining, b_val) = le_u8(remaining)?;
            (remaining, [a, r_val, g_val, b_val])
        } else {
            (remaining, [255, 255, 255, 255])
        };

        tiles.push(GndTile {
            u1,
            u2,
            u3,
            u4,
            v1,
            v2,
            v3,
            v4,
            texture,
            color,
        });
        current_input = remaining;
    }

    Ok((current_input, tiles))
}

fn parse_surfaces(input: &[u8], width: u32, height: u32) -> IResult<&[u8], Vec<GndSurface>> {
    let count = (width * height) as usize;
    let mut surfaces = Vec::with_capacity(count);
    let mut current_input = input;

    for _ in 0..count {
        let (remaining, h1) = le_f32(current_input)?;
        let (remaining, h2) = le_f32(remaining)?;
        let (remaining, h3) = le_f32(remaining)?;
        let (remaining, h4) = le_f32(remaining)?;
        let (remaining, tile_up) = le_i32(remaining)?;
        let (remaining, tile_front) = le_i32(remaining)?;
        let (remaining, tile_right) = le_i32(remaining)?;

        surfaces.push(GndSurface {
            height: [h1, h2, h3, h4],
            tile_up,
            tile_front,
            tile_right,
        });
        current_input = remaining;
    }

    Ok((current_input, surfaces))
}

fn parse_gnd(input: &[u8]) -> IResult<&[u8], RoGround> {
    let (input, version) = parse_header(input)?;
    let (input, width) = le_u32(input)?;
    let (input, height) = le_u32(input)?;
    let (input, _) = le_f32(input)?;
    let (input, (textures, texture_indexes)) = parse_textures(input)?;
    let (input, _) = parse_lightmap(input)?; // We parse it just to move the input forward
    let (input, tile_count) = le_u32(input)?;
    let (input, tiles) = parse_tiles(input, tile_count, &version)?;
    let (input, surfaces) = parse_surfaces(input, width, height)?;

    Ok((
        input,
        RoGround {
            version,
            width,
            height,
            textures,
            texture_indexes,
            tiles,
            surfaces,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_header() {
        let data = b"GRGN\x01\x07";
        let (_, version) = parse_header(data).unwrap();
        assert_eq!(version, "1.7");
    }
}
