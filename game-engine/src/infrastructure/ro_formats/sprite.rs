use nom::{
    bytes::complete::{tag, take},
    number::complete::{le_i16, le_u16, le_u8},
    IResult,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SpriteError {
    #[error("Invalid SPR header")]
    InvalidHeader,
    #[error("Unsupported SPR version: {0}")]
    UnsupportedVersion(String),
    #[error("Parse error: {0}")]
    ParseError(String),
}

#[derive(Debug, Clone)]
pub struct RoSprite {
    pub version: f32,
    pub indexed_count: u16,
    pub rgba_count: u16,
    pub frames: Vec<SpriteFrame>,
    pub palette: Option<Palette>,
}

#[derive(Debug, Clone)]
pub struct SpriteFrame {
    pub width: u16,
    pub height: u16,
    pub data: Vec<u8>,
    pub is_rgba: bool,
}

#[derive(Debug, Clone)]
pub struct Palette {
    pub colors: Vec<[u8; 4]>, // RGBA
}

pub fn parse_spr(data: &[u8]) -> Result<RoSprite, SpriteError> {
    let (mut remaining_data, (version, indexed_count, rgba_count)) = parse_header(data)
        .map_err(|e| SpriteError::ParseError(format!("Header parse error: {e:?}")))?;

    let total_frames = indexed_count + rgba_count;
    let mut frames = Vec::with_capacity(total_frames as usize);

    // Parse indexed frames first - use different methods based on version
    for _ in 0..indexed_count {
        let (new_data, frame) = if version < 2.1 {
            parse_indexed_frame(remaining_data)
                .map_err(|e| SpriteError::ParseError(format!("Indexed frame error: {e:?}")))?
        } else {
            parse_indexed_frame_rle(remaining_data)
                .map_err(|e| SpriteError::ParseError(format!("Indexed RLE frame error: {e:?}")))?
        };
        frames.push(frame);
        remaining_data = new_data;
    }

    // Parse RGBA frames
    for _ in 0..rgba_count {
        let (new_data, frame) = parse_rgba_frame(remaining_data)
            .map_err(|e| SpriteError::ParseError(format!("RGBA frame error: {e:?}")))?;
        frames.push(frame);
        remaining_data = new_data;
    }

    // Parse palette from end of file if we have indexed frames and version > 1.0
    let palette = if indexed_count > 0 && version > 1.0 {
        if data.len() >= 1024 {
            // 256 colors * 4 bytes (RGB + reserved byte)
            let palette_data = &data[data.len() - 1024..];

            let mut colors = Vec::with_capacity(256);
            for (i, chunk) in palette_data.chunks(4).enumerate() {
                if chunk.len() >= 4 {
                    // RO palette format: RGB + reserved byte (not alpha)
                    // Index 0 is reserved for transparency
                    // All other colors should be fully opaque (alpha=255)
                    let alpha = if i == 0 { 0 } else { 255 };
                    colors.push([chunk[0], chunk[1], chunk[2], alpha]);
                }
            }
            Some(Palette { colors })
        } else {
            None
        }
    } else {
        None
    };

    Ok(RoSprite {
        version,
        indexed_count,
        rgba_count,
        frames,
        palette,
    })
}

fn parse_header(data: &[u8]) -> IResult<&[u8], (f32, u16, u16)> {
    let (data, _signature) = tag("SP")(data)?;
    let (data, version_major) = le_u8(data)?;
    let (data, version_minor) = le_u8(data)?;

    // Convert to version format like roBrowser: major + minor/10
    let version = version_major as f32 / 10.0 + version_minor as f32;

    let (data, indexed_count) = le_u16(data)?;

    // RGBA count only exists in version > 1.1
    let (data, rgba_count) = if version > 1.1 {
        le_u16(data)?
    } else {
        (data, 0)
    };

    Ok((data, (version, indexed_count, rgba_count)))
}

fn parse_indexed_frame(data: &[u8]) -> IResult<&[u8], SpriteFrame> {
    let (data, width) = le_u16(data)?;
    let (data, height) = le_u16(data)?;
    let data_size = (width as usize) * (height as usize);
    let (data, pixel_data) = take(data_size)(data)?;

    Ok((
        data,
        SpriteFrame {
            width,
            height,
            data: pixel_data.to_vec(),
            is_rgba: false,
        },
    ))
}

fn parse_indexed_frame_rle(data: &[u8]) -> IResult<&[u8], SpriteFrame> {
    let (data, width) = le_u16(data)?;
    let (data, height) = le_u16(data)?;
    let (data, data_size) = le_u16(data)?;
    let (data, compressed_data) = take(data_size as usize)(data)?;

    // Decompress RLE data
    let mut decompressed_data = Vec::with_capacity((width as usize) * (height as usize));
    let mut i = 0;

    while i < compressed_data.len() {
        let c = compressed_data[i];
        decompressed_data.push(c);
        i += 1;

        if c == 0 && i < compressed_data.len() {
            let count = compressed_data[i];
            i += 1;

            if count == 0 {
                decompressed_data.push(0);
            } else {
                // Repeat the zero byte (count - 1) more times
                for _ in 1..count {
                    decompressed_data.push(0);
                }
            }
        }
    }

    Ok((
        data,
        SpriteFrame {
            width,
            height,
            data: decompressed_data,
            is_rgba: false,
        },
    ))
}

fn parse_rgba_frame(data: &[u8]) -> IResult<&[u8], SpriteFrame> {
    let (data, width_signed) = le_i16(data)?;
    let (data, height_signed) = le_i16(data)?;

    // Convert signed to unsigned, handle negative values
    let width = width_signed.unsigned_abs();
    let height = height_signed.unsigned_abs();

    let data_size = (width as usize) * (height as usize) * 4; // RGBA = 4 bytes per pixel
    let (data, pixel_data) = take(data_size)(data)?;

    Ok((
        data,
        SpriteFrame {
            width,
            height,
            data: pixel_data.to_vec(),
            is_rgba: true,
        },
    ))
}
