use crate::infrastructure::assets::converters::apply_magenta_transparency;
use bevy::{
    asset::{io::Reader, AssetLoader, LoadContext},
    prelude::*,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat},
    },
};
use thiserror::Error;

#[derive(Default)]
pub struct BmpLoader;

#[derive(Debug, Error)]
pub enum BmpLoaderError {
    #[error("Could not load BMP: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid BMP format: {0}")]
    InvalidFormat(String),
}

impl AssetLoader for BmpLoader {
    type Asset = Image;
    type Settings = ();
    type Error = BmpLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        // Parse BMP header
        if bytes.len() < 54 {
            return Err(BmpLoaderError::InvalidFormat("File too small".into()));
        }

        // Check BMP signature
        if &bytes[0..2] != b"BM" {
            return Err(BmpLoaderError::InvalidFormat(
                "Invalid BMP signature".into(),
            ));
        }

        // Read header values
        let data_offset = u32::from_le_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]) as usize;
        let width = i32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]);
        let height = i32::from_le_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]);
        let bits_per_pixel = u16::from_le_bytes([bytes[28], bytes[29]]);

        if width <= 0 || height <= 0 {
            return Err(BmpLoaderError::InvalidFormat("Invalid dimensions".into()));
        }

        let width = width as u32;
        let height = height.unsigned_abs();
        let bottom_up = height as i32 > 0;

        // Convert to RGBA based on bits per pixel
        let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);

        match bits_per_pixel {
            8 => {
                // 8-bit indexed color with palette
                // Read palette (located after header, 1024 bytes for 256 colors * 4 bytes each)
                let palette_offset = 54; // Standard BMP header size
                let mut palette = Vec::with_capacity(256);

                for i in 0..256 {
                    let idx = palette_offset + i * 4;
                    if idx + 3 < bytes.len() {
                        let b = bytes[idx];
                        let g = bytes[idx + 1];
                        let r = bytes[idx + 2];
                        palette.push([r, g, b, 255]);
                    } else {
                        palette.push([0, 0, 0, 255]);
                    }
                }

                // Calculate row size (rows are padded to 4-byte boundaries)
                let row_size = width.div_ceil(4) * 4;

                for y in 0..height {
                    let source_y = if bottom_up { height - 1 - y } else { y };

                    let row_start = data_offset + (source_y * row_size) as usize;

                    for x in 0..width {
                        let pixel_offset = row_start + x as usize;

                        if pixel_offset < bytes.len() {
                            let palette_index = bytes[pixel_offset] as usize;
                            if palette_index < palette.len() {
                                rgba_data.extend_from_slice(&palette[palette_index]);
                            } else {
                                rgba_data.extend_from_slice(&[255, 0, 255, 255]);
                                // Magenta for errors
                            }
                        }
                    }
                }
            }
            24 => {
                // 24-bit BGR
                let row_size = (width * 3).div_ceil(4) * 4;

                for y in 0..height {
                    let source_y = if bottom_up { height - 1 - y } else { y };

                    let row_start = data_offset + (source_y * row_size) as usize;

                    for x in 0..width {
                        let pixel_offset = row_start + (x * 3) as usize;

                        if pixel_offset + 2 < bytes.len() {
                            let b = bytes[pixel_offset];
                            let g = bytes[pixel_offset + 1];
                            let r = bytes[pixel_offset + 2];

                            rgba_data.push(r);
                            rgba_data.push(g);
                            rgba_data.push(b);
                            rgba_data.push(255); // Alpha
                        }
                    }
                }
            }
            _ => {
                return Err(BmpLoaderError::InvalidFormat(format!(
                    "Unsupported bits per pixel: {bits_per_pixel}",
                )));
            }
        }

        // Apply magenta transparency (RGB 255, 0, 255 becomes transparent)
        apply_magenta_transparency(&mut rgba_data);

        Ok(Image::new(
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            rgba_data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        ))
    }

    fn extensions(&self) -> &[&str] {
        &["bmp", "BMP"]
    }
}
