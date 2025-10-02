use crate::utils::string_utils::parse_korean_string;
use bevy::log::error;
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
            Ok((_, gnd)) => Ok(gnd),
            Err(e) => {
                error!("GND parse error: {:?}", e);
                Err(GndError::ParseError(e.to_string()))
            }
        }
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
            height: [h1 / 5.0, h2 / 5.0, h3 / 5.0, h4 / 5.0],
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
