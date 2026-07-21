use bevy::{
    asset::{AssetLoader, LoadContext, RenderAssetUsages, io::Reader},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use thiserror::Error;

#[derive(Default, TypePath)]
pub struct TgaLoader;

#[derive(Debug, Error)]
pub enum TgaLoaderError {
    #[error("Could not load TGA: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not decode TGA: {0}")]
    Decode(#[from] image::ImageError),
}

/// Decodes a TGA byte buffer into a Bevy RGBA8 `Image`.
///
/// RO status-icon TGAs are uncompressed true-color, 32x32, 32bpp with 8-bit
/// alpha; the `image` crate's TGA decoder also handles 24bpp (opaque alpha)
/// and respects the image-descriptor byte's vertical-origin bit.
fn decode_tga(bytes: &[u8]) -> Result<Image, TgaLoaderError> {
    let rgba = image::load_from_memory_with_format(bytes, image::ImageFormat::Tga)?.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();

    Ok(Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba.into_raw(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ))
}

impl AssetLoader for TgaLoader {
    type Asset = Image;
    type Settings = ();
    type Error = TgaLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        decode_tga(&bytes)
    }

    fn extensions(&self) -> &[&str] {
        &["tga", "TGA"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a minimal uncompressed 32x32 32bpp true-color TGA with a single
    /// distinguishable semi-transparent pixel, bottom-up (origin bit unset).
    fn build_test_tga() -> Vec<u8> {
        const WIDTH: u16 = 32;
        const HEIGHT: u16 = 32;

        let mut bytes = Vec::with_capacity(18 + WIDTH as usize * HEIGHT as usize * 4);

        bytes.push(0); // id length
        bytes.push(0); // color map type
        bytes.push(2); // image type: uncompressed true-color
        bytes.extend_from_slice(&[0; 5]); // color map spec
        bytes.extend_from_slice(&0u16.to_le_bytes()); // x origin
        bytes.extend_from_slice(&0u16.to_le_bytes()); // y origin
        bytes.extend_from_slice(&WIDTH.to_le_bytes());
        bytes.extend_from_slice(&HEIGHT.to_le_bytes());
        bytes.push(32); // bits per pixel
        bytes.push(0x08); // descriptor: 8-bit alpha, origin bit unset (bottom-up)

        // Pixel data is stored bottom-to-top, left-to-right, BGRA per pixel.
        // Place the marker pixel in the first file scanline (bottom of the
        // image) so a correct vertical-origin flip puts it at the last row
        // of the decoded top-down image.
        for row in 0..HEIGHT {
            for col in 0..WIDTH {
                if row == 0 && col == 5 {
                    bytes.extend_from_slice(&[10, 20, 30, 128]); // B,G,R,A
                } else {
                    bytes.extend_from_slice(&[0, 0, 0, 255]);
                }
            }
        }

        bytes
    }

    #[test]
    fn decodes_32bit_tga_with_correct_channels_and_origin() {
        let bytes = build_test_tga();
        let image = decode_tga(&bytes).expect("decode");

        assert_eq!(image.texture_descriptor.size.width, 32);
        assert_eq!(image.texture_descriptor.size.height, 32);
        assert_eq!(
            image.texture_descriptor.format,
            TextureFormat::Rgba8UnormSrgb
        );

        let data = image.data.expect("image data");
        let width = 32usize;
        let marker_row = 31; // bottom-up file row 0 -> last row of top-down image
        let marker_col = 5;
        let offset = (marker_row * width + marker_col) * 4;

        assert_eq!(&data[offset..offset + 4], &[30, 20, 10, 128]);
    }
}
