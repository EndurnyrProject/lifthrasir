use crate::infrastructure::assets::loaders::RoPaletteAsset;
use crate::infrastructure::ro_formats::{sprite::SpriteFrame, Palette};
use crate::utils::SPRITE_SCALE_SMALL;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use std::collections::HashSet;

/// Convert indexed sprite data to RGBA using palette
pub fn convert_indexed_to_rgba(indexed_data: &[u8], palette: &Palette) -> Vec<u8> {
    let mut rgba_data = Vec::with_capacity(indexed_data.len() * 4);
    let mut invalid_indices = 0;
    let mut transparent_pixels = 0;
    let mut unique_indices = HashSet::new();

    for &index in indexed_data {
        unique_indices.insert(index);

        if let Some(color) = palette.colors.get(index as usize) {
            // Check if this color is magenta (255, 0, 255)
            let is_magenta = color[0] == 255 && color[1] == 0 && color[2] == 255;

            // In RO sprites, index 0 OR magenta color is transparent
            let final_color = if index == 0 || is_magenta {
                [color[0], color[1], color[2], 0] // Transparent
            } else {
                [color[0], color[1], color[2], 255] // Opaque
            };

            if final_color[3] == 0 {
                transparent_pixels += 1;
            }
            rgba_data.extend_from_slice(&final_color);
        } else {
            // Magenta for missing palette entries (but transparent)
            rgba_data.extend_from_slice(&[255, 0, 255, 0]);
            invalid_indices += 1;
        }
    }

    // Log conversion stats
    if unique_indices.len() > 1 {
        debug!(
            "Palette conversion: {} unique indices, {:.1}% transparent",
            unique_indices.len(),
            transparent_pixels as f32 / indexed_data.len() as f32 * 100.0
        );
    }

    if invalid_indices > 0 {
        warn!(
            "Found {} invalid palette indices (will show as transparent magenta)",
            invalid_indices
        );
    }

    rgba_data
}

/// Convert indexed sprite data to RGBA using a custom RoPaletteAsset
/// This is used for custom palettes like hair colors
/// Handles transparency for index 0 and magenta (255, 0, 255)
pub fn convert_indexed_to_rgba_with_custom_palette(
    indexed_data: &[u8],
    palette: &RoPaletteAsset,
) -> Vec<u8> {
    let mut rgba_data = Vec::with_capacity(indexed_data.len() * 4);

    for &index in indexed_data {
        if let Some(color) = palette.colors.get(index as usize) {
            // Check for magenta transparency marker (255, 0, 255)
            let is_magenta = color[0] == 255 && color[1] == 0 && color[2] == 255;

            // Index 0 OR magenta color = transparent
            let final_color = if index == 0 || is_magenta {
                [color[0], color[1], color[2], 0] // Transparent
            } else {
                *color // Opaque (already RGBA with alpha)
            };

            rgba_data.extend_from_slice(&final_color);
        } else {
            // Fallback for invalid palette indices - transparent
            rgba_data.extend_from_slice(&[0, 0, 0, 0]);
        }
    }

    rgba_data
}

/// Convert a sprite frame to RGBA, handling both indexed and RGBA formats
/// Supports custom palettes for hair colors and other customizations
pub fn convert_sprite_frame_to_rgba(
    frame: &SpriteFrame,
    default_palette: Option<&Palette>,
    custom_palette: Option<&RoPaletteAsset>,
) -> Vec<u8> {
    if frame.is_rgba {
        // Already RGBA format - return as-is
        frame.data.clone()
    } else if let Some(custom_pal) = custom_palette {
        // Use custom palette (for hair colors, etc)
        convert_indexed_to_rgba_with_custom_palette(&frame.data, custom_pal)
    } else if let Some(default_pal) = default_palette {
        // Use default palette from sprite file
        convert_indexed_to_rgba(&frame.data, default_pal)
    } else {
        // Fallback: grayscale conversion
        frame
            .data
            .iter()
            .flat_map(|&pixel| {
                if pixel == 0 {
                    [0, 0, 0, 0] // Transparent
                } else {
                    [pixel, pixel, pixel, 255] // Opaque grayscale
                }
            })
            .collect()
    }
}

/// Create a Bevy Image from sprite frame data
pub fn create_bevy_image(width: u32, height: u32, mut rgba_data: Vec<u8>) -> Image {
    // Apply magenta transparency to RGBA sprites
    apply_magenta_transparency(&mut rgba_data);

    Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba_data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// Calculate appropriate scale for sprite based on size
pub fn calculate_sprite_scale(width: u32, height: u32) -> f32 {
    if width < crate::utils::SPRITE_SIZE_THRESHOLD || height < crate::utils::SPRITE_SIZE_THRESHOLD {
        SPRITE_SCALE_SMALL
    } else {
        1.0
    }
}

/// Apply magenta transparency to RGBA image data
/// In Ragnarok Online, magenta (255, 0, 255) is treated as transparent
/// Uses tolerance to catch near-magenta colors from filtering/compression
pub fn apply_magenta_transparency(rgba_data: &mut [u8]) {
    // Thresholds for near-magenta detection
    // Broad thresholds to catch variations from texture filtering/compression
    const MAGENTA_THRESHOLD: u8 = 240; // R and B should be >= 240 (was 250)
    const GREEN_THRESHOLD: u8 = 15; // G should be <= 15 (was 5)

    // Process every 4 bytes (RGBA)
    for pixel in rgba_data.chunks_exact_mut(4) {
        // Check if this pixel is close to magenta
        // R >= 240, G <= 15, B >= 240
        let is_near_magenta = pixel[0] >= MAGENTA_THRESHOLD
            && pixel[1] <= GREEN_THRESHOLD
            && pixel[2] >= MAGENTA_THRESHOLD;

        if is_near_magenta {
            // Zero out RGB and alpha to make it fully transparent
            // Setting RGB to 0 prevents color bleeding in rendering
            pixel[0] = 0; // R
            pixel[1] = 0; // G
            pixel[2] = 0; // B
            pixel[3] = 0; // A
        }
    }
}

/// Decode image data from bytes based on file extension
/// Supports BMP, TGA, JPG, and PNG formats commonly used in RO
pub fn decode_image_from_bytes(
    data: &[u8],
    filename: &str,
) -> Result<Image, Box<dyn std::error::Error>> {
    use image::ImageFormat;

    // Determine format from filename extension
    let format = if filename.ends_with(".bmp") || filename.ends_with(".BMP") {
        ImageFormat::Bmp
    } else if filename.ends_with(".tga") || filename.ends_with(".TGA") {
        ImageFormat::Tga
    } else if filename.ends_with(".jpg")
        || filename.ends_with(".JPG")
        || filename.ends_with(".jpeg")
        || filename.ends_with(".JPEG")
    {
        ImageFormat::Jpeg
    } else if filename.ends_with(".png") || filename.ends_with(".PNG") {
        ImageFormat::Png
    } else {
        ImageFormat::Bmp
    };

    let img = image::load_from_memory_with_format(data, format)?;
    let rgba = img.to_rgba8();
    let dimensions = rgba.dimensions();

    let mut rgba_data = rgba.into_raw();
    apply_magenta_transparency(&mut rgba_data);

    let mut bevy_image = Image::new(
        Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba_data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );

    // Set sampler to repeat for tiling textures like water
    bevy_image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..Default::default()
    });

    Ok(bevy_image)
}
