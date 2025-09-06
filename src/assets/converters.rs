use bevy::{
    prelude::*,
    render::{
        render_resource::{Extent3d, TextureDimension, TextureFormat},
        render_asset::RenderAssetUsages,
    },
};
use std::collections::HashSet;
use crate::ro_formats::Palette;
use crate::utils::SPRITE_SCALE_SMALL;

/// Convert indexed sprite data to RGBA using palette
pub fn convert_indexed_to_rgba(indexed_data: &[u8], palette: &Palette) -> Vec<u8> {
    let mut rgba_data = Vec::with_capacity(indexed_data.len() * 4);
    let mut invalid_indices = 0;
    let mut transparent_pixels = 0;
    let mut unique_indices = HashSet::new();
    
    for &index in indexed_data {
        unique_indices.insert(index);
        
        if let Some(color) = palette.colors.get(index as usize) {
            // In RO sprites, index 0 is typically transparent, others are opaque
            let final_color = if index == 0 {
                [color[0], color[1], color[2], 0]  // Index 0 transparent
            } else {
                [color[0], color[1], color[2], 255] // All other indices opaque
            };
            
            if final_color[3] == 0 {
                transparent_pixels += 1;
            }
            rgba_data.extend_from_slice(&final_color);
        } else {
            // Magenta for missing palette entries
            rgba_data.extend_from_slice(&[255, 0, 255, 255]);
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
        warn!("Found {} invalid palette indices (will show as magenta)", invalid_indices);
    }
    
    rgba_data
}

/// Create a Bevy Image from sprite frame data
pub fn create_bevy_image(
    width: u32,
    height: u32,
    rgba_data: Vec<u8>,
) -> Image {
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