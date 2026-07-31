use anyhow::ensure;
use image::RgbaImage;

const MAGIC: [u8; 12] = [
    0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
];
const ZSTD_LEVEL: i32 = 19;
const HEADER_LEN: usize = 80;
const LEVEL_INDEX_LEN: usize = 24;
const DFD_LEN: usize = 92;

struct Mip {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

pub fn encode_ktx2(rgba: &RgbaImage, srgb: bool) -> anyhow::Result<Vec<u8>> {
    ensure!(
        rgba.width() > 0 && rgba.height() > 0,
        "KTX2 images must have non-zero dimensions"
    );

    let mips = mip_chain(rgba, srgb);
    let compressed = mips
        .iter()
        .map(|mip| zstd::bulk::compress(&mip.pixels, ZSTD_LEVEL))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(write_ktx2(
        rgba.width(),
        rgba.height(),
        srgb,
        &mips,
        &compressed,
    ))
}

fn mip_chain(image: &RgbaImage, srgb: bool) -> Vec<Mip> {
    let mut mips = vec![Mip {
        width: image.width(),
        height: image.height(),
        pixels: image.as_raw().clone(),
    }];

    while let Some(current) = mips.last().filter(|mip| mip.width > 1 || mip.height > 1) {
        let width = (current.width / 2).max(1);
        let height = (current.height / 2).max(1);
        mips.push(Mip {
            width,
            height,
            pixels: downsample(current, width, height, srgb),
        });
    }
    mips
}

fn downsample(src: &Mip, dst_width: u32, dst_height: u32, srgb: bool) -> Vec<u8> {
    let mut dst = vec![0; (dst_width * dst_height * 4) as usize];
    for y in 0..dst_height {
        for x in 0..dst_width {
            let samples = [
                (x * 2, y * 2),
                ((x * 2 + 1).min(src.width - 1), y * 2),
                (x * 2, (y * 2 + 1).min(src.height - 1)),
                (
                    (x * 2 + 1).min(src.width - 1),
                    (y * 2 + 1).min(src.height - 1),
                ),
            ];
            let pixel = filter_pixel(src, samples, srgb);
            let offset = ((y * dst_width + x) * 4) as usize;
            dst[offset..offset + 4].copy_from_slice(&pixel);
        }
    }
    dst
}

fn filter_pixel(src: &Mip, samples: [(u32, u32); 4], srgb: bool) -> [u8; 4] {
    let mut weighted_rgb = [0.0; 3];
    let mut plain_rgb = [0.0; 3];
    let mut alpha_sum = 0.0;

    for (x, y) in samples {
        let offset = ((y * src.width + x) * 4) as usize;
        let alpha = src.pixels[offset + 3] as f32 / 255.0;
        alpha_sum += alpha;
        for channel in 0..3 {
            let value = decode_channel(src.pixels[offset + channel], srgb);
            plain_rgb[channel] += value;
            weighted_rgb[channel] += value * alpha;
        }
    }

    let rgb = if alpha_sum > 0.0 {
        weighted_rgb.map(|value| value / alpha_sum)
    } else {
        plain_rgb.map(|value| value / 4.0)
    };
    [
        encode_channel(rgb[0], srgb),
        encode_channel(rgb[1], srgb),
        encode_channel(rgb[2], srgb),
        (alpha_sum / 4.0 * 255.0).round() as u8,
    ]
}

fn decode_channel(value: u8, srgb: bool) -> f32 {
    let value = value as f32 / 255.0;
    if !srgb {
        return value;
    }
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn encode_channel(value: f32, srgb: bool) -> u8 {
    let value = if !srgb {
        value
    } else if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn write_ktx2(
    width: u32,
    height: u32,
    srgb: bool,
    mips: &[Mip],
    compressed: &[Vec<u8>],
) -> Vec<u8> {
    let dfd_offset = HEADER_LEN + LEVEL_INDEX_LEN * mips.len();
    let payload_offset = align(dfd_offset + DFD_LEN, 8);
    let mut offsets = vec![0; mips.len()];
    let mut cursor = payload_offset;
    for level in (0..mips.len()).rev() {
        cursor = align(cursor, 8);
        offsets[level] = cursor;
        cursor += compressed[level].len();
    }

    let mut out = Vec::with_capacity(cursor);
    out.extend_from_slice(&MAGIC);
    push_u32(&mut out, if srgb { 43 } else { 37 });
    for value in [
        1,
        width,
        height,
        0,
        0,
        1,
        mips.len() as u32,
        2,
        dfd_offset as u32,
        DFD_LEN as u32,
        0,
        0,
    ] {
        push_u32(&mut out, value);
    }
    push_u64(&mut out, 0);
    push_u64(&mut out, 0);

    for (level, mip) in mips.iter().enumerate() {
        push_u64(&mut out, offsets[level] as u64);
        push_u64(&mut out, compressed[level].len() as u64);
        push_u64(&mut out, mip.pixels.len() as u64);
    }
    out.extend_from_slice(&dfd(srgb));
    out.resize(payload_offset, 0);
    for level in (0..mips.len()).rev() {
        out.resize(offsets[level], 0);
        out.extend_from_slice(&compressed[level]);
    }
    out
}

fn dfd(srgb: bool) -> [u8; DFD_LEN] {
    let mut out = Vec::with_capacity(DFD_LEN);
    push_u32(&mut out, DFD_LEN as u32);
    push_u32(&mut out, 0);
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&88u16.to_le_bytes());
    out.extend_from_slice(&[1, 1, if srgb { 2 } else { 1 }, 0]);
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&[4, 0, 0, 0, 0, 0, 0, 0]);
    for (channel, bit_offset) in [(0, 0u16), (1, 8), (2, 16), (15, 24)] {
        out.extend_from_slice(&bit_offset.to_le_bytes());
        out.extend_from_slice(&[7, channel, 0, 0, 0, 0]);
        push_u32(&mut out, 0);
        push_u32(&mut out, 255);
    }
    out.try_into().expect("DFD has a fixed size")
}

fn align(value: usize, alignment: usize) -> usize {
    value.div_ceil(alignment) * alignment
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::encode_ktx2;
    use image::{Rgba, RgbaImage};
    use ktx2::{Format, SupercompressionScheme};

    fn decoded_levels(bytes: &[u8]) -> Vec<Vec<u8>> {
        ktx2::Reader::new(bytes)
            .unwrap()
            .levels()
            .map(|level| {
                let decoded =
                    zstd::bulk::decompress(level.data, level.uncompressed_byte_length as usize)
                        .unwrap();
                assert_eq!(decoded.len() as u64, level.uncompressed_byte_length);
                decoded
            })
            .collect()
    }

    #[test]
    fn encodes_parseable_rgba8_ktx2_with_expected_mips() {
        let image = RgbaImage::from_fn(2, 2, |x, y| match (x, y) {
            (0, 0) => Rgba([0, 0, 0, 255]),
            (1, 0) => Rgba([100, 100, 100, 255]),
            (0, 1) => Rgba([200, 200, 200, 255]),
            _ => Rgba([255, 255, 255, 255]),
        });
        let bytes = encode_ktx2(&image, false).unwrap();
        let reader = ktx2::Reader::new(&bytes).unwrap();
        let header = reader.header();

        assert_eq!(header.format, Some(Format::R8G8B8A8_UNORM));
        assert_eq!(
            header.supercompression_scheme,
            Some(SupercompressionScheme::Zstandard)
        );
        assert_eq!(
            (header.level_count, header.face_count, header.layer_count),
            (2, 1, 0)
        );
        let basic_dfd = reader.basic_dfd().unwrap();
        assert_eq!(
            basic_dfd
                .texel_block_dimensions
                .map(|dimension| dimension.get()),
            [1; 4]
        );
        assert_eq!(basic_dfd.bytes_planes, [4, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(
            decoded_levels(&bytes),
            vec![image.into_raw(), vec![139, 139, 139, 255]]
        );
    }

    #[test]
    fn alpha_weights_rgb_without_dark_transparent_bleed() {
        let image = RgbaImage::from_fn(2, 1, |x, _| {
            if x == 0 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });
        assert_eq!(
            decoded_levels(&encode_ktx2(&image, false).unwrap())[1],
            [255, 0, 0, 128]
        );
    }

    #[test]
    fn odd_dimensions_reach_one_by_one() {
        let image = RgbaImage::from_pixel(5, 3, Rgba([7, 8, 9, 10]));
        let bytes = encode_ktx2(&image, false).unwrap();
        let reader = ktx2::Reader::new(&bytes).unwrap();
        assert_eq!(
            (reader.header().pixel_width, reader.header().pixel_height),
            (5, 3)
        );
        assert_eq!(reader.header().level_count, 3);
        assert_eq!(
            decoded_levels(&bytes)
                .iter()
                .map(Vec::len)
                .collect::<Vec<_>>(),
            [60, 8, 4]
        );

        let one = encode_ktx2(&RgbaImage::from_pixel(1, 1, Rgba([1, 2, 3, 4])), false).unwrap();
        assert_eq!(ktx2::Reader::new(&one).unwrap().header().level_count, 1);
    }

    #[test]
    fn averages_srgb_in_linear_space() {
        let image =
            RgbaImage::from_fn(2, 1, |x, _| Rgba([if x == 0 { 0 } else { 255 }, 0, 0, 255]));
        let bytes = encode_ktx2(&image, true).unwrap();
        let reader = ktx2::Reader::new(&bytes).unwrap();
        assert_eq!(reader.header().format, Some(Format::R8G8B8A8_SRGB));
        assert_eq!(decoded_levels(&bytes)[1], [188, 0, 0, 255]);
    }

    #[test]
    fn encoding_is_deterministic() {
        let image = RgbaImage::from_pixel(3, 5, Rgba([42, 23, 99, 127]));
        assert_eq!(
            encode_ktx2(&image, true).unwrap(),
            encode_ktx2(&image, true).unwrap()
        );
    }
}
