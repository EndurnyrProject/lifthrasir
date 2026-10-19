use crate::{CELL_SIZE, string_utils::parse_korean_string};
use glam::Vec3;
use nom::{
    IResult, Parser,
    bytes::complete::{tag, take},
    number::complete::{le_f32, le_i32, le_u8, le_u16, le_u32},
};
use thiserror::Error;
use tracing::{debug, error};

#[derive(Debug, Error)]
pub enum GndError {
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error(
        "GND version {version} left {actual} unconsumed byte(s), expected {expected}; \
         the layout is out of sync with the file"
    )]
    TrailingBytes {
        version: String,
        actual: usize,
        expected: usize,
    },
}

/// Encoded `major << 8 | minor`, mirroring BrowEdit's `0x0107`-style constants.
///
/// The version used to be compared as a `String`, which orders "1.10" *before*
/// "1.7" - correct today only because no minor has reached double digits.
pub type GndVersion = u16;

const V1_7: GndVersion = 0x0107;
const V1_8: GndVersion = 0x0108;
const V1_9: GndVersion = 0x0109;

/// One water zone's parameters.
///
/// From GND 1.8 the water configuration lives here rather than in the RSW, and
/// takes precedence over anything the RSW says.
#[derive(Debug, Clone, PartialEq)]
pub struct GndWaterZone {
    pub level: f32,
    pub water_type: i32,
    pub wave_height: f32,
    pub wave_speed: f32,
    pub wave_pitch: f32,
    pub anim_speed: i32,
}

/// The map's water, as a `split_width` x `split_height` grid of zones.
///
/// Most maps are a single 1x1 zone covering everything, but some split the map
/// into a grid where each cell has its own level and wave parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct GndWater {
    pub split_width: u32,
    pub split_height: u32,
    /// Row-major, `split_height` rows of `split_width` zones.
    pub zones: Vec<GndWaterZone>,
}

impl GndWater {
    /// The zone covering surface cell `(x, y)` of a `width` x `height` map.
    ///
    /// Zones tile the map evenly, so the cell index is scaled into zone space.
    pub fn zone_at(&self, x: usize, y: usize, width: u32, height: u32) -> &GndWaterZone {
        let zone_x = zone_index(x, width, self.split_width);
        let zone_y = zone_index(y, height, self.split_height);
        &self.zones[zone_y * self.split_width as usize + zone_x]
    }
}

fn zone_index(cell: usize, cells: u32, splits: u32) -> usize {
    if cells == 0 || splits == 0 {
        return 0;
    }
    ((cell * splits as usize) / cells as usize).min(splits as usize - 1)
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
    /// `major << 8 | minor`, the form the version gates are compared against.
    pub raw_version: GndVersion,
    pub width: u32,
    pub height: u32,
    pub textures: Vec<String>,
    pub texture_indexes: Vec<usize>,
    pub tiles: Vec<GndTile>,
    pub surfaces: Vec<GndSurface>,
    /// Present from version 1.8. When set it supersedes the RSW's water block,
    /// which those versions no longer carry.
    pub water: Option<GndWater>,
}

impl RoGround {
    pub fn from_bytes(input: &[u8]) -> Result<Self, GndError> {
        let (remaining, gnd) = parse_gnd(input).map_err(|e| {
            error!("GND parse error: {:?}", e);
            GndError::ParseError(e.to_string())
        })?;

        // The GND ends on its last section, so anything left over means the
        // layout drifted and every field beyond that point is suspect.
        if !remaining.is_empty() {
            return Err(GndError::TrailingBytes {
                version: gnd.version.clone(),
                actual: remaining.len(),
                expected: 0,
            });
        }

        debug!(
            "Parsed GND: version={}, width={}, height={}, surfaces={}, water={}",
            gnd.version,
            gnd.width,
            gnd.height,
            gnd.surfaces.len(),
            gnd.water.is_some()
        );
        Ok(gnd)
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

fn parse_header(input: &[u8]) -> IResult<&[u8], (u8, u8)> {
    let (input, _) = tag(&b"GRGN"[..])(input)?;
    let (input, major) = le_u8(input)?;
    let (input, minor) = le_u8(input)?;
    Ok((input, (major, minor)))
}

/// Water block, present from version 1.8.
///
/// A default parameter set is followed by the zone grid. Below 1.9 a zone only
/// overrides the level and inherits the rest of the defaults.
fn parse_water(input: &[u8], version: GndVersion) -> IResult<&[u8], GndWater> {
    let (input, level) = le_f32(input)?;
    let (input, water_type) = le_i32(input)?;
    let (input, wave_height) = le_f32(input)?;
    let (input, wave_speed) = le_f32(input)?;
    let (input, wave_pitch) = le_f32(input)?;
    let (input, anim_speed) = le_i32(input)?;

    let defaults = GndWaterZone {
        level,
        water_type,
        wave_height,
        wave_speed,
        wave_pitch,
        anim_speed,
    };

    let (input, split_width) = le_u32(input)?;
    let (input, split_height) = le_u32(input)?;

    let zone_count = split_width as usize * split_height as usize;
    let mut zones = Vec::with_capacity(zone_count);
    let mut remaining = input;

    for _ in 0..zone_count {
        let (rest, level) = le_f32(remaining)?;
        let (rest, zone) = if version >= V1_9 {
            let (rest, water_type) = le_i32(rest)?;
            let (rest, wave_height) = le_f32(rest)?;
            let (rest, wave_speed) = le_f32(rest)?;
            let (rest, wave_pitch) = le_f32(rest)?;
            let (rest, anim_speed) = le_i32(rest)?;
            (
                rest,
                GndWaterZone {
                    level,
                    water_type,
                    wave_height,
                    wave_speed,
                    wave_pitch,
                    anim_speed,
                },
            )
        } else {
            (
                rest,
                GndWaterZone {
                    level,
                    ..defaults.clone()
                },
            )
        };
        zones.push(zone);
        remaining = rest;
    }

    Ok((
        remaining,
        GndWater {
            split_width,
            split_height,
            zones,
        },
    ))
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

fn parse_tiles(input: &[u8], count: u32, version: GndVersion) -> IResult<&[u8], Vec<GndTile>> {
    let mut tiles = Vec::with_capacity(count as usize);
    let mut current_input = input;

    for _ in 0..count {
        let (remaining, (u1, u2, u3, u4)) =
            (le_f32, le_f32, le_f32, le_f32).parse(current_input)?;
        let (remaining, (v1, v2, v3, v4)) = (le_f32, le_f32, le_f32, le_f32).parse(remaining)?;
        let (remaining, texture) = le_u16(remaining)?;
        let (remaining, _) = le_u16(remaining)?; // Light, we have our own better lightmaps

        let (remaining, color) = if version >= V1_7 {
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
    let (input, (major, minor)) = parse_header(input)?;
    let version = ((major as GndVersion) << 8) | minor as GndVersion;
    let (input, width) = le_u32(input)?;
    let (input, height) = le_u32(input)?;
    let (input, _) = le_f32(input)?;
    let (input, (textures, texture_indexes)) = parse_textures(input)?;
    let (input, _) = parse_lightmap(input)?; // We parse it just to move the input forward
    let (input, tile_count) = le_u32(input)?;
    let (input, tiles) = parse_tiles(input, tile_count, version)?;
    let (input, surfaces) = parse_surfaces(input, width, height)?;

    let (input, water) = if version >= V1_8 {
        let (input, water) = parse_water(input, version)?;
        (input, Some(water))
    } else {
        (input, None)
    };

    Ok((
        input,
        RoGround {
            version: format!("{major}.{minor}"),
            raw_version: version,
            width,
            height,
            textures,
            texture_indexes,
            tiles,
            surfaces,
            water,
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
        assert_eq!(version, (1, 7));
    }
}
