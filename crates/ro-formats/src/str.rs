use crate::string_utils::parse_korean_string;
use glam::Vec2;
use nom::{
    IResult, Parser,
    bytes::complete::{tag, take},
    multi::count,
    number::complete::{le_f32, le_i32, le_u8, le_u32},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StrError {
    #[error("Parse error: {0}")]
    ParseError(String),
}

#[derive(Debug, Clone)]
pub struct StrEffect {
    pub fps: u32,
    pub max_key: u32,
    pub layers: Vec<StrLayer>,
}

#[derive(Debug, Clone)]
pub struct StrLayer {
    pub texture_names: Vec<String>,
    pub frames: Vec<StrFrame>,
}

#[derive(Debug, Clone)]
pub struct StrFrame {
    pub frame_index: i32,
    pub frame_type: i32,
    pub offset: Vec2,
    pub uv: [f32; 8],
    pub xy: [f32; 8],
    pub texture_index: f32,
    pub animation_type: i32,
    pub delay: f32,
    pub angle: f32,
    pub color: [f32; 4],
    pub src_blend: i32,
    pub dst_blend: i32,
    pub mt_present: i32,
}

impl StrEffect {
    pub fn from_bytes(input: &[u8]) -> Result<Self, StrError> {
        match parse_str(input) {
            Ok((_, effect)) => Ok(effect),
            Err(e) => Err(StrError::ParseError(e.to_string())),
        }
    }
}

fn parse_str(input: &[u8]) -> IResult<&[u8], StrEffect> {
    let (input, _signature) = tag(&b"STRM"[..])(input)?;
    let (input, _version_major) = le_u8(input)?;
    let (input, _version_minor) = le_u8(input)?;
    let (input, _skip0) = take(2usize)(input)?;
    let (input, fps) = le_u32(input)?;
    let (input, max_key) = le_u32(input)?;
    let (input, layer_count) = le_u32(input)?;
    let (input, _skip1) = take(16usize)(input)?;
    let (input, layers) = count(parse_layer, layer_count as usize).parse(input)?;

    Ok((
        input,
        StrEffect {
            fps,
            max_key,
            layers,
        },
    ))
}

fn parse_layer(input: &[u8]) -> IResult<&[u8], StrLayer> {
    let (input, texture_count) = le_i32(input)?;
    let (input, texture_names) = count(
        |i| parse_korean_string(i, 128),
        texture_count.max(0) as usize,
    )
    .parse(input)?;
    let (input, frame_count) = le_i32(input)?;
    let (input, frames) = count(parse_frame, frame_count.max(0) as usize).parse(input)?;

    Ok((
        input,
        StrLayer {
            texture_names,
            frames,
        },
    ))
}

fn parse_frame(input: &[u8]) -> IResult<&[u8], StrFrame> {
    let (input, frame_index) = le_i32(input)?;
    let (input, frame_type) = le_i32(input)?;
    let (input, offset_x) = le_f32(input)?;
    let (input, offset_y) = le_f32(input)?;
    let (input, uv) = parse_f32_array8(input)?;
    let (input, xy) = parse_f32_array8(input)?;
    let (input, texture_index) = le_f32(input)?;
    let (input, animation_type) = le_i32(input)?;
    let (input, delay) = le_f32(input)?;
    let (input, angle) = le_f32(input)?;
    let (input, c0) = le_f32(input)?;
    let (input, c1) = le_f32(input)?;
    let (input, c2) = le_f32(input)?;
    let (input, c3) = le_f32(input)?;
    let (input, src_blend) = le_i32(input)?;
    let (input, dst_blend) = le_i32(input)?;
    let (input, mt_present) = le_i32(input)?;

    Ok((
        input,
        StrFrame {
            frame_index,
            frame_type,
            offset: Vec2::new(offset_x, offset_y),
            uv,
            xy,
            texture_index,
            animation_type,
            delay,
            angle,
            color: [c0, c1, c2, c3],
            src_blend,
            dst_blend,
            mt_present,
        },
    ))
}

fn parse_f32_array8(input: &[u8]) -> IResult<&[u8], [f32; 8]> {
    let (input, values) = count(le_f32, 8).parse(input)?;
    let mut array = [0.0f32; 8];
    array.copy_from_slice(&values);
    Ok((input, array))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_name(name: &str) -> Vec<u8> {
        let mut buf = vec![0u8; 128];
        let bytes = name.as_bytes();
        buf[..bytes.len()].copy_from_slice(bytes);
        buf
    }

    fn build_fixture() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(b"STRM");
        data.push(0x02); // version major
        data.push(0x00); // version minor
        data.extend_from_slice(&[0u8; 2]); // skip0
        data.extend_from_slice(&30u32.to_le_bytes()); // fps
        data.extend_from_slice(&60u32.to_le_bytes()); // max_key
        data.extend_from_slice(&1u32.to_le_bytes()); // layer_count
        data.extend_from_slice(&[0u8; 16]); // skip1

        // layer
        data.extend_from_slice(&1i32.to_le_bytes()); // texture_count
        data.extend_from_slice(&fixed_name("fire.bmp")); // texture name
        data.extend_from_slice(&1i32.to_le_bytes()); // frame_count

        // frame
        data.extend_from_slice(&0i32.to_le_bytes()); // frame_index
        data.extend_from_slice(&1i32.to_le_bytes()); // frame_type
        data.extend_from_slice(&1.0f32.to_le_bytes()); // offset.x
        data.extend_from_slice(&2.0f32.to_le_bytes()); // offset.y
        for i in 0..8 {
            data.extend_from_slice(&(i as f32 * 0.1).to_le_bytes()); // uv
        }
        for i in 0..8 {
            data.extend_from_slice(&(i as f32 + 10.0).to_le_bytes()); // xy
        }
        data.extend_from_slice(&3.0f32.to_le_bytes()); // texture_index
        data.extend_from_slice(&5i32.to_le_bytes()); // animation_type
        data.extend_from_slice(&0.5f32.to_le_bytes()); // delay
        data.extend_from_slice(&512.0f32.to_le_bytes()); // angle
        data.extend_from_slice(&255.0f32.to_le_bytes()); // color r
        data.extend_from_slice(&128.0f32.to_le_bytes()); // color g
        data.extend_from_slice(&64.0f32.to_le_bytes()); // color b
        data.extend_from_slice(&255.0f32.to_le_bytes()); // color a
        data.extend_from_slice(&5i32.to_le_bytes()); // src_blend
        data.extend_from_slice(&1i32.to_le_bytes()); // dst_blend
        data.extend_from_slice(&0i32.to_le_bytes()); // mt_present

        data
    }

    #[test]
    fn test_parse_str_fixture() {
        let data = build_fixture();
        let effect = StrEffect::from_bytes(&data).expect("should parse");

        assert_eq!(effect.fps, 30);
        assert_eq!(effect.max_key, 60);
        assert_eq!(effect.layers.len(), 1);

        let layer = &effect.layers[0];
        assert_eq!(layer.texture_names, vec!["fire.bmp".to_string()]);
        assert_eq!(layer.frames.len(), 1);

        let frame = &layer.frames[0];
        assert_eq!(frame.frame_index, 0);
        assert_eq!(frame.frame_type, 1);
        assert_eq!(frame.offset, Vec2::new(1.0, 2.0));
        assert_eq!(frame.uv, [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7]);
        assert_eq!(frame.xy, [10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0]);
        assert_eq!(frame.texture_index, 3.0);
        assert_eq!(frame.color, [255.0, 128.0, 64.0, 255.0]);
        assert_eq!(frame.src_blend, 5);
        assert_eq!(frame.dst_blend, 1);
        assert_eq!(frame.mt_present, 0);
    }

    #[test]
    fn test_wrong_signature_errors() {
        let data = b"XXXX\x02\x00\x00\x00";
        assert!(matches!(
            StrEffect::from_bytes(data),
            Err(StrError::ParseError(_))
        ));
    }

    #[test]
    fn test_truncated_errors() {
        let mut data = build_fixture();
        data.truncate(20);
        assert!(StrEffect::from_bytes(&data).is_err());
    }
}
