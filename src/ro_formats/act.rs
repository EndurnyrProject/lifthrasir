use nom::{
    IResult,
    bytes::complete::take,
    error::Error as NomError,
    number::complete::{le_f32, le_i32, le_u8, le_u16, le_u32},
};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct RoAction {
    pub version: f32,
    pub actions: Vec<ActionSequence>,
    pub sounds: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ActionSequence {
    pub animations: Vec<Animation>,
    pub delay: f32,
}

#[derive(Debug, Clone)]
pub struct Animation {
    pub layers: Vec<Layer>,
    pub sound_id: i32,
    pub positions: Vec<Position>,
}

#[derive(Debug, Clone)]
pub struct Layer {
    pub pos: [i32; 2],
    pub sprite_index: i32,
    pub is_mirror: bool,
    pub scale: [f32; 2],
    pub color: [f32; 4],
    pub angle: i32,
    pub sprite_type: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Error)]
pub enum ActError {
    #[error("Invalid ACT header")]
    InvalidHeader,
    #[error("Unsupported version: {0}")]
    UnsupportedVersion(f32),
    #[error("Parse error: {0}")]
    ParseError(String),
}

fn parse_header(input: &[u8]) -> IResult<&[u8], (String, f32), NomError<&[u8]>> {
    let (input, signature) = take(2usize)(input)?;
    let signature = String::from_utf8_lossy(signature).to_string();

    let (input, version_major) = le_u8(input)?;
    let (input, version_minor) = le_u8(input)?;

    let version = (version_major as f32) / 10.0 + (version_minor as f32);

    Ok((input, (signature, version)))
}

fn parse_layer(input: &[u8], version: f32) -> IResult<&[u8], Layer, NomError<&[u8]>> {
    let (input, pos_x) = le_i32(input)?;
    let (input, pos_y) = le_i32(input)?;
    let (input, sprite_index) = le_i32(input)?;
    let (input, is_mirror) = le_i32(input)?;

    let mut layer = Layer {
        pos: [pos_x, pos_y],
        sprite_index,
        is_mirror: is_mirror != 0,
        scale: [1.0, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
        angle: 0,
        sprite_type: 0,
        width: 0,
        height: 0,
    };

    let input = if version >= 2.0 {
        let (input, color_r) = le_u8(input)?;
        let (input, color_g) = le_u8(input)?;
        let (input, color_b) = le_u8(input)?;
        let (input, color_a) = le_u8(input)?;

        layer.color = [
            color_r as f32 / 255.0,
            color_g as f32 / 255.0,
            color_b as f32 / 255.0,
            color_a as f32 / 255.0,
        ];

        let (input, scale_x) = le_f32(input)?;
        let (input, scale_y) = if version <= 2.3 {
            (input, scale_x)
        } else {
            le_f32(input)?
        };

        layer.scale = [scale_x, scale_y];

        let (input, angle) = le_i32(input)?;
        let (input, sprite_type) = le_i32(input)?;

        layer.angle = angle;
        layer.sprite_type = sprite_type;

        if version >= 2.5 {
            let (input, width) = le_i32(input)?;
            let (input, height) = le_i32(input)?;
            layer.width = width;
            layer.height = height;
            input
        } else {
            input
        }
    } else {
        input
    };

    Ok((input, layer))
}

fn parse_animation_impl(input: &[u8], version: f32) -> IResult<&[u8], Animation, NomError<&[u8]>> {
    let (input, _) = take(32usize)(input)?; // Unknown bytes
    let (input, layer_count) = le_u32(input)?;

    // Parse layers manually
    let mut remaining = input;
    let mut layers = Vec::new();
    for _ in 0..layer_count {
        let (new_remaining, layer) = parse_layer(remaining, version)?;
        layers.push(layer);
        remaining = new_remaining;
    }

    let (input, sound_id) = if version >= 2.0 {
        le_i32(remaining)?
    } else {
        (remaining, -1)
    };

    let (input, positions) = if version >= 2.3 {
        let (input, pos_count) = le_u32(input)?;
        let mut remaining = input;
        let mut positions = Vec::new();
        for _ in 0..pos_count {
            let (new_remaining, _) = le_u32(remaining)?; // Unknown
            let (new_remaining, x) = le_i32(new_remaining)?;
            let (new_remaining, y) = le_i32(new_remaining)?;
            let (new_remaining, _) = le_u32(new_remaining)?; // Unknown
            positions.push(Position { x, y });
            remaining = new_remaining;
        }
        (remaining, positions)
    } else {
        (input, Vec::new())
    };

    Ok((
        input,
        Animation {
            layers,
            sound_id,
            positions,
        },
    ))
}

fn parse_action_sequence_impl(
    input: &[u8],
    version: f32,
) -> IResult<&[u8], ActionSequence, NomError<&[u8]>> {
    let (input, animation_count) = le_u32(input)?;

    // Parse animations manually
    let mut remaining = input;
    let mut animations = Vec::new();
    for _ in 0..animation_count {
        let (new_remaining, animation) = parse_animation_impl(remaining, version)?;
        animations.push(animation);
        remaining = new_remaining;
    }

    Ok((
        remaining,
        ActionSequence {
            animations,
            delay: 150.0, // Default delay, may be overridden later
        },
    ))
}

fn parse_sounds(input: &[u8]) -> IResult<&[u8], Vec<String>, NomError<&[u8]>> {
    let (input, sound_count) = le_u32(input)?;

    let mut remaining = input;
    let mut sounds = Vec::new();
    for _ in 0..sound_count {
        let (new_remaining, sound_bytes) = take(40usize)(remaining)?;
        let sound_name = String::from_utf8_lossy(sound_bytes)
            .trim_end_matches('\0')
            .to_string();
        sounds.push(sound_name);
        remaining = new_remaining;
    }

    Ok((remaining, sounds))
}

pub fn parse_act(data: &[u8]) -> Result<RoAction, ActError> {
    let (input, (signature, version)) = parse_header(data)
        .map_err(|_| ActError::ParseError("Failed to parse header".to_string()))?;

    if signature != "AC" {
        return Err(ActError::InvalidHeader);
    }

    if !(2.0..=2.5).contains(&version) {
        return Err(ActError::UnsupportedVersion(version));
    }

    let (input, action_count) = le_u16::<&[u8], NomError<&[u8]>>(input)
        .map_err(|_| ActError::ParseError("Failed to parse action count".to_string()))?;

    let (input, _) = take::<usize, &[u8], NomError<&[u8]>>(10usize)(input)
        .map_err(|_| ActError::ParseError("Failed to skip unknown bytes".to_string()))?;

    // Parse actions manually
    let mut remaining = input;
    let mut actions = Vec::new();
    for _ in 0..action_count {
        let (new_remaining, action) = parse_action_sequence_impl(remaining, version)
            .map_err(|_| ActError::ParseError("Failed to parse action sequence".to_string()))?;
        actions.push(action);
        remaining = new_remaining;
    }

    let (input, sounds) = if version >= 2.1 {
        parse_sounds(remaining)
            .map_err(|_| ActError::ParseError("Failed to parse sounds".to_string()))?
    } else {
        (remaining, Vec::new())
    };

    if version >= 2.2 && !input.is_empty() {
        for (i, action) in actions.iter_mut().enumerate() {
            if let Ok((_, delay)) = le_f32::<&[u8], NomError<&[u8]>>(&input[i * 4..]) {
                action.delay = delay * 25.0;
            }
        }
    }

    Ok(RoAction {
        version,
        actions,
        sounds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_header() {
        let data = b"AC\x02\x01";
        let result = parse_header(data);
        assert!(result.is_ok());
        let (_, (signature, version)) = result.unwrap();
        assert_eq!(signature, "AC");
        assert_eq!(version, 2.1);
    }

    #[test]
    fn test_invalid_header() {
        let data = b"XX\x02\x01";
        let result = parse_act(data);
        assert!(matches!(result, Err(ActError::InvalidHeader)));
    }
}
