//! Offline KTX2 output for runtime textures.
//!
//! Textures are block-compressed to BC7 through `ctt` (the same library Bevy's
//! `CompressedImageSaver` uses), which also generates the mip chain and writes
//! the KTX2 container with zstd supercompression. BC7 is 4x smaller in VRAM
//! than the uncompressed RGBA this used to write; the runtime picks
//! `Bc7RgbaUnormSrgb` up from `CompressedImageFormats::BC`.

use anyhow::{Context, ensure};
use ctt::{
    AlphaMode, ColorSpace, Container, ConvertSettings, Format, Image, MipmapFilter, PipelineOutput,
    Quality, Surface, TargetFormat, TextureKind, convert, encoders::Encoder,
};
use image::RgbaImage;

pub fn encode_ktx2(rgba: &RgbaImage, srgb: bool) -> anyhow::Result<Vec<u8>> {
    ensure!(
        rgba.width() > 0 && rgba.height() > 0,
        "KTX2 images must have non-zero dimensions"
    );

    let surface = Surface {
        data: rgba.as_raw().clone(),
        width: rgba.width(),
        height: rgba.height(),
        depth: 1,
        stride: rgba.width() * 4,
        slice_stride: 0,
        format: if srgb {
            Format::R8G8B8A8_SRGB
        } else {
            Format::R8G8B8A8_UNORM
        },
        color_space: if srgb {
            ColorSpace::Srgb
        } else {
            ColorSpace::Linear
        },
        // Straight alpha in and out: the materials sampling these textures are
        // `AlphaMode::Mask`, and premultiplying would darken cutout edges.
        alpha: AlphaMode::Straight,
    };

    let output = convert(
        Image {
            surfaces: vec![vec![surface]],
            kind: TextureKind::Texture2D,
        },
        ConvertSettings {
            format: Some(TargetFormat::Compressed {
                format: Format::BC7_UNORM_BLOCK,
                encoder: Encoder::Auto,
            }),
            container: Container::ktx2_zstd(0),
            quality: Quality::default(),
            mipmap: true,
            mipmap_filter: MipmapFilter::Lanczos3,
            ..ConvertSettings::default()
        },
    )
    .context("encoding BC7 KTX2")?;

    match output {
        PipelineOutput::Encoded(bytes) => Ok(bytes),
        other => anyhow::bail!("expected an encoded KTX2 container, got {other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::encode_ktx2;
    use image::{Rgba, RgbaImage};
    use ktx2::{Format, SupercompressionScheme};

    #[test]
    fn encodes_bc7_ktx2_with_a_full_mip_chain() {
        let image = RgbaImage::from_pixel(8, 4, Rgba([200, 100, 50, 255]));
        let bytes = encode_ktx2(&image, true).unwrap();
        let reader = ktx2::Reader::new(&bytes).unwrap();
        let header = reader.header();

        assert_eq!(header.format, Some(Format::BC7_SRGB_BLOCK));
        assert_eq!(
            header.supercompression_scheme,
            Some(SupercompressionScheme::Zstandard)
        );
        assert_eq!(
            (header.pixel_width, header.pixel_height),
            (8, 4),
            "mip 0 keeps the source extent"
        );
        assert_eq!(header.level_count, 4, "8x4 halves down to 1x1");

        // Base level first, one 16-byte BC7 block per 4x4 tile: 8x4 is two
        // blocks, every smaller level fits in one.
        let block_counts = [2, 1, 1, 1].map(|blocks| blocks * 16);
        assert_eq!(
            reader
                .levels()
                .map(|level| level.uncompressed_byte_length as usize)
                .collect::<Vec<_>>(),
            block_counts.to_vec()
        );
    }

    #[test]
    fn tags_linear_textures_as_unorm() {
        let bytes = encode_ktx2(&RgbaImage::from_pixel(4, 4, Rgba([1, 2, 3, 4])), false).unwrap();
        assert_eq!(
            ktx2::Reader::new(&bytes).unwrap().header().format,
            Some(Format::BC7_UNORM_BLOCK)
        );
    }

    #[test]
    fn encoding_is_deterministic() {
        let image = RgbaImage::from_pixel(9, 5, Rgba([42, 23, 99, 127]));
        assert_eq!(
            encode_ktx2(&image, true).unwrap(),
            encode_ktx2(&image, true).unwrap()
        );
    }

    #[test]
    fn rejects_empty_images() {
        assert!(encode_ktx2(&RgbaImage::new(0, 0), true).is_err());
    }
}
