use crate::infrastructure::ro_formats::Palette;
use crate::utils::SPRITE_SCALE_SMALL;
use bevy::{
    prelude::*,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat},
    },
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
pub fn apply_magenta_transparency(rgba_data: &mut [u8]) {
    // Process every 4 bytes (RGBA)
    for pixel in rgba_data.chunks_exact_mut(4) {
        // Check if this pixel is magenta (R=255, G=0, B=255)
        if pixel[0] == 255 && pixel[1] == 0 && pixel[2] == 255 {
            // Set alpha to 0 to make it transparent
            pixel[3] = 0;
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
    use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
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
