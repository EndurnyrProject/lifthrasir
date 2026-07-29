use crate::string_utils::parse_korean_string;
use nom::{
    IResult, Parser,
    bytes::complete::tag,
    number::complete::{le_f32, le_u8, le_u32},
};
use thiserror::Error;
use tracing::{debug, error, warn};

#[derive(Debug, Error)]
pub enum RswError {
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error(
        "RSW version {version} left {actual} unconsumed byte(s), expected {expected}; \
         the layout is out of sync with the file"
    )]
    TrailingBytes {
        version: String,
        actual: usize,
        expected: usize,
    },
}

/// Encoded `major << 8 | minor`, mirroring BrowEdit's `0x0206`-style constants.
pub type RswVersion = u16;

const V1_3: RswVersion = 0x0103;
const V1_4: RswVersion = 0x0104;
const V1_5: RswVersion = 0x0105;
const V1_6: RswVersion = 0x0106;
const V1_7: RswVersion = 0x0107;
const V1_8: RswVersion = 0x0108;
const V1_9: RswVersion = 0x0109;
const V2_0: RswVersion = 0x0200;
const V2_1: RswVersion = 0x0201;
const V2_2: RswVersion = 0x0202;
const V2_5: RswVersion = 0x0205;
const V2_6: RswVersion = 0x0206;
const V2_7: RswVersion = 0x0207;

/// The quadtree appended after the object list from version 2.1 onwards.
///
/// It is a fixed, fully populated tree: 1 + 4 + 16 + 64 + 256 + 1024 = 1365
/// nodes, each holding four `vec3`s (max, min, halfSize, center).
const QUADTREE_NODES: usize = 1365;
const QUADTREE_BYTES: usize = QUADTREE_NODES * 4 * 3 * 4;

#[derive(Debug, Clone)]
pub struct RswWater {
    pub level: f32,
    pub water_type: u32,
    pub wave_height: f32,
    pub wave_speed: f32,
    pub wave_pitch: f32,
    pub anim_speed: u32,
}

impl Default for RswWater {
    fn default() -> Self {
        Self {
            level: 0.0,
            water_type: 0,
            wave_height: 0.2,
            wave_speed: 2.0,
            wave_pitch: 50.0,
            anim_speed: 3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RswLight {
    pub longitude: u32,
    pub latitude: u32,
    pub diffuse: [f32; 3],
    pub ambient: [f32; 3],
    pub opacity: f32,
}

impl Default for RswLight {
    fn default() -> Self {
        Self {
            longitude: 45,
            latitude: 45,
            diffuse: [1.0, 1.0, 1.0],
            ambient: [0.3, 0.3, 0.3],
            opacity: 1.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RswSound {
    pub name: String,
    pub wav_file: String,
    pub position: [f32; 3],
    pub volume: f32,
    pub width: u32,
    pub height: u32,
    pub range: f32,
    pub cycle: f32,
}

#[derive(Debug, Clone)]
pub struct RswLightObj {
    pub name: String,
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub range: f32,
}

#[derive(Debug, Clone)]
pub struct RswEffect {
    pub name: String,
    pub position: [f32; 3],
    pub effect_type: u32,
    pub emit_speed: f32,
    pub params: [f32; 4],
}

#[derive(Debug, Clone)]
pub enum RswObject {
    Model(RswModel),
    Light(RswLightObj),
    Sound(RswSound),
    Effect(RswEffect),
}

#[derive(Debug, Clone)]
pub struct RswModel {
    pub name: String,
    pub anim_type: u32,
    pub anim_speed: f32,
    pub block_type: u32,
    pub filename: String,
    pub node_name: String,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}

#[derive(Debug, Clone)]
pub struct RoWorld {
    pub version: String,
    /// `major << 8 | minor`, the form the version gates are compared against.
    pub raw_version: RswVersion,
    /// Client build number, present from version 2.5. Some object fields are
    /// gated on it rather than on the version alone.
    pub build_number: i32,
    pub ini_file: String,
    pub gnd_file: String,
    pub gat_file: String,
    pub src_file: Option<String>,
    pub water: RswWater,
    pub light: RswLight,
    pub ground: RswGround,
    pub objects: Vec<RswObject>,
}

#[derive(Debug, Clone)]
pub struct RswGround {
    pub top: i32,
    pub bottom: i32,
    pub left: i32,
    pub right: i32,
}

impl Default for RswGround {
    fn default() -> Self {
        Self {
            top: -500,
            bottom: 500,
            left: -500,
            right: 500,
        }
    }
}

impl RoWorld {
    pub fn from_bytes(input: &[u8]) -> Result<Self, RswError> {
        let (remaining, rsw) = parse_rsw(input).map_err(|e| {
            error!("RSW parse error: {e:?}");
            RswError::ParseError(e.to_string())
        })?;

        // A desynchronised layout still "parses", it just stops in the wrong
        // place and reinterprets unrelated bytes as floats - which is how NaN
        // positions and garbage lighting reached the converted maps. Requiring
        // the parse to land exactly on the trailing quadtree makes that loud.
        let expected = expected_trailing_bytes(rsw.raw_version);
        if remaining.len() != expected {
            return Err(RswError::TrailingBytes {
                version: rsw.version.clone(),
                actual: remaining.len(),
                expected,
            });
        }

        debug!("RSW parsed successfully, version {}", rsw.version);
        Ok(rsw)
    }
}

/// Bytes the format legitimately leaves behind after the object list.
fn expected_trailing_bytes(version: RswVersion) -> usize {
    if version >= V2_1 { QUADTREE_BYTES } else { 0 }
}

/// Read a non-negative element count.
///
/// Counts are stored signed; `for _ in 0..count` is an empty range when the
/// value is negative, so garbage from a desynchronised parse would silently
/// yield an empty list instead of an error.
fn parse_count(input: &[u8]) -> IResult<&[u8], usize> {
    let (input, count) = nom::number::complete::le_i32(input)?;
    let count = usize::try_from(count).map_err(|_| {
        nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Verify))
    })?;
    Ok((input, count))
}

fn parse_header(input: &[u8]) -> IResult<&[u8], (u8, u8)> {
    let (input, _) = tag(&b"GRSW"[..])(input)?;
    let (input, major) = le_u8(input)?;
    let (input, minor) = le_u8(input)?;
    Ok((input, (major, minor)))
}

fn parse_water(input: &[u8], version: RswVersion) -> IResult<&[u8], RswWater> {
    let mut water = RswWater::default();
    let input = if version >= V1_3 {
        let (input, level) = le_f32(input)?;
        water.level = level;
        input
    } else {
        input
    };

    let input = if version >= V1_8 {
        let (input, water_type) = le_u32(input)?;
        let (input, wave_height) = le_f32(input)?;
        let (input, wave_speed) = le_f32(input)?;
        let (input, wave_pitch) = le_f32(input)?;
        water.water_type = water_type;
        water.wave_height = wave_height;
        water.wave_speed = wave_speed;
        water.wave_pitch = wave_pitch;
        input
    } else {
        input
    };

    let input = if version >= V1_9 {
        let (input, anim_speed) = le_u32(input)?;
        water.anim_speed = anim_speed;
        input
    } else {
        input
    };

    Ok((input, water))
}

fn parse_light(input: &[u8], version: RswVersion) -> IResult<&[u8], RswLight> {
    let mut light = RswLight::default();
    let input = if version >= V1_5 {
        let (input, longitude) = le_u32(input)?;
        let (input, latitude) = le_u32(input)?;
        let (input, (dr, dg, db)) = (le_f32, le_f32, le_f32).parse(input)?;
        let (input, (ar, ag, ab)) = (le_f32, le_f32, le_f32).parse(input)?;
        light.longitude = longitude;
        light.latitude = latitude;
        light.diffuse = [dr, dg, db];
        light.ambient = [ar, ag, ab];
        input
    } else {
        input
    };

    let input = if version >= V1_7 {
        let (input, opacity) = le_f32(input)?;
        light.opacity = opacity;
        input
    } else {
        input
    };

    Ok((input, light))
}

fn parse_ground(input: &[u8], version: RswVersion) -> IResult<&[u8], RswGround> {
    if version >= V1_6 {
        let (input, top) = le_u32(input)?;
        let (input, bottom) = le_u32(input)?;
        let (input, left) = le_u32(input)?;
        let (input, right) = le_u32(input)?;
        Ok((
            input,
            RswGround {
                top: top as i32,
                bottom: bottom as i32,
                left: left as i32,
                right: right as i32,
            },
        ))
    } else {
        Ok((input, RswGround::default()))
    }
}

fn parse_model(input: &[u8], version: RswVersion, build_number: i32) -> IResult<&[u8], RswModel> {
    let (input, name) = if version >= V1_3 {
        parse_korean_string(input, 40)?
    } else {
        (input, String::new())
    };

    let (input, anim_type) = if version >= V1_3 {
        le_u32(input)?
    } else {
        (input, 0)
    };

    let (input, anim_speed) = if version >= V1_3 {
        le_f32(input)?
    } else {
        (input, 0.0)
    };

    let (input, block_type) = if version >= V1_3 {
        le_u32(input)?
    } else {
        (input, 0)
    };

    // Gated on the build number, not on the version alone: 2.6 files written by
    // an older build do not carry this byte.
    let (input, _) = if version >= V2_6 && build_number >= 186 {
        let (input, _) = le_u8(input)?;
        (input, ())
    } else {
        (input, ())
    };

    let (input, _) = if version >= V2_7 {
        let (input, _) = le_u32(input)?;
        (input, ())
    } else {
        (input, ())
    };

    let (input, filename) = parse_korean_string(input, 80)?;
    let (input, mut node_name) = parse_korean_string(input, 80)?;

    // Validate node_name - check for corrupted data
    // Corrupted names are usually very short and contain non-ASCII characters
    if !node_name.is_empty()
        && (node_name.len() < 3
            || !node_name
                .chars()
                .all(|c| c.is_ascii_graphic() || c.is_ascii_whitespace()))
    {
        debug!(
            "Detected corrupted node_name '{}' (bytes: {:?}) for model '{}', clearing it",
            node_name,
            node_name.as_bytes(),
            filename
        );
        node_name.clear(); // Clear corrupted node name, will use main node instead
    }

    let (input, (px, py, pz)) = (le_f32, le_f32, le_f32).parse(input)?;
    let (input, (rx, ry, rz)) = (le_f32, le_f32, le_f32).parse(input)?;
    let (input, (sx, sy, sz)) = (le_f32, le_f32, le_f32).parse(input)?;

    Ok((
        input,
        RswModel {
            name,
            anim_type,
            anim_speed,
            block_type,
            filename,
            node_name,
            position: [px, py, pz],
            rotation: [rx, ry, rz],
            scale: [sx, sy, sz],
        },
    ))
}

fn parse_objects(
    input: &[u8],
    count: usize,
    version: RswVersion,
    build_number: i32,
) -> IResult<&[u8], Vec<RswObject>> {
    let mut objects = Vec::with_capacity(count);
    let mut current_input = input;

    for i in 0..count {
        let (remaining, obj_type) = le_u32(current_input)?;
        debug!("  Object {}: type {}", i, obj_type);

        let (remaining, object) = match obj_type {
            1 => {
                // Model
                let (remaining, model) = parse_model(remaining, version, build_number)?;
                (remaining, RswObject::Model(model))
            }
            2 => {
                // Light
                let (remaining, name) = parse_korean_string(remaining, 80)?;
                let (remaining, (x, y, z)) = (le_f32, le_f32, le_f32).parse(remaining)?;
                let (remaining, (r, g, b)) = (le_f32, le_f32, le_f32).parse(remaining)?;
                let (remaining, range) = le_f32(remaining)?;

                (
                    remaining,
                    RswObject::Light(RswLightObj {
                        name,
                        position: [x, y, z],
                        color: [r, g, b],
                        range,
                    }),
                )
            }
            3 => {
                // Sound
                let (remaining, name) = parse_korean_string(remaining, 80)?;
                let (remaining, wav_file) = parse_korean_string(remaining, 80)?;
                let (remaining, (x, y, z)) = (le_f32, le_f32, le_f32).parse(remaining)?;
                let (remaining, volume) = le_f32(remaining)?;
                let (remaining, width) = le_u32(remaining)?;
                let (remaining, height) = le_u32(remaining)?;
                let (remaining, range) = le_f32(remaining)?;
                let (remaining, cycle) = if version >= V2_0 {
                    le_f32(remaining)?
                } else {
                    (remaining, 0.0)
                };

                (
                    remaining,
                    RswObject::Sound(RswSound {
                        name,
                        wav_file,
                        position: [x, y, z],
                        volume,
                        width,
                        height,
                        range,
                        cycle,
                    }),
                )
            }
            4 => {
                // Effect
                let (remaining, name) = parse_korean_string(remaining, 80)?;
                let (remaining, (x, y, z)) = (le_f32, le_f32, le_f32).parse(remaining)?;
                let (remaining, effect_type) = le_u32(remaining)?;
                let (remaining, emit_speed) = le_f32(remaining)?;
                let (remaining, (p1, p2, p3, p4)) =
                    (le_f32, le_f32, le_f32, le_f32).parse(remaining)?;

                (
                    remaining,
                    RswObject::Effect(RswEffect {
                        name,
                        position: [x, y, z],
                        effect_type,
                        emit_speed,
                        params: [p1, p2, p3, p4],
                    }),
                )
            }
            // Object records are variable length, so an unrecognised tag means
            // the cursor is no longer on an object boundary. There is no way to
            // skip forward to the next one, and continuing would reread the same
            // bytes and silently drop the rest of the map.
            _ => {
                warn!("  Unknown object type {obj_type} at index {i}");
                return Err(nom::Err::Failure(nom::error::Error::new(
                    current_input,
                    nom::error::ErrorKind::Tag,
                )));
            }
        };

        objects.push(object);
        current_input = remaining;
    }

    Ok((current_input, objects))
}

fn parse_rsw(input: &[u8]) -> IResult<&[u8], RoWorld> {
    let (input, (major, minor)) = parse_header(input)?;
    let version = ((major as RswVersion) << 8) | minor as RswVersion;

    // Build number precedes the 2.2 pad byte.
    let (input, build_number) = if version >= V2_5 {
        nom::number::complete::le_i32(input)?
    } else {
        (input, 0)
    };

    let (input, _) = if version >= V2_2 {
        let (input, _) = le_u8(input)?;
        (input, ())
    } else {
        (input, ())
    };

    let (input, ini_file) = parse_korean_string(input, 40)?;
    let (input, gnd_file) = parse_korean_string(input, 40)?;

    let (input, gat_file) = if version > V1_4 {
        parse_korean_string(input, 40)?
    } else {
        (input, gnd_file.clone())
    };

    // The client writes the ini filename a second time here; the value is
    // redundant but the 40 bytes are not.
    let (input, src_file) = parse_korean_string(input, 40)?;

    // From 2.6 the water configuration lives in the GND instead, and this block
    // is absent. `RoGround::water` supersedes this field for those maps.
    let (input, water) = if version < V2_6 {
        parse_water(input, version)?
    } else {
        (input, RswWater::default())
    };

    let (input, light) = parse_light(input, version)?;
    let (input, ground) = parse_ground(input, version)?;

    // Purpose unknown; present from 2.7 and must be skipped to reach the objects.
    let input = if version >= V2_7 {
        let (input, unknown_count) = parse_count(input)?;
        let (input, _) = nom::bytes::complete::take(unknown_count * 4)(input)?;
        input
    } else {
        input
    };

    let (input, object_count) = parse_count(input)?;
    let (input, objects) = parse_objects(input, object_count, version, build_number)?;

    Ok((
        input,
        RoWorld {
            version: format!("{major}.{minor}"),
            raw_version: version,
            build_number,
            ini_file,
            gnd_file,
            gat_file,
            src_file: Some(src_file),
            water,
            light,
            ground,
            objects,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_header() {
        let data = b"GRSW\x02\x05";
        let (_, version) = parse_header(data).unwrap();
        assert_eq!(version, (2, 5));
    }
}
