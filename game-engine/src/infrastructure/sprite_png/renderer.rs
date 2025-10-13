use super::{
    error::SpritePngError,
    types::{SpritePngRequest, SpritePngResponse},
};
use crate::infrastructure::{
    assets::{
        converters::convert_sprite_frame_to_rgba, hierarchical_manager::HierarchicalAssetManager,
    },
    ro_formats::{parse_act, parse_spr},
};
use image::{imageops::FilterType, ImageFormat, RgbaImage};
use std::io::Cursor;

/// Headless sprite renderer that generates PNG images without Bevy context
pub struct SpriteRenderer {
    asset_manager: HierarchicalAssetManager,
}

impl SpriteRenderer {
    /// Create a new sprite renderer with the given asset manager
    pub fn new(asset_manager: HierarchicalAssetManager) -> Self {
        Self { asset_manager }
    }

    /// Render a sprite frame to PNG format
    pub fn render_to_png(
        &self,
        request: &SpritePngRequest,
    ) -> Result<SpritePngResponse, SpritePngError> {
        // 1. Normalize and load sprite file
        let sprite_path = Self::normalize_path(&request.sprite_path);
        let sprite_data = self
            .asset_manager
            .load(&sprite_path)
            .map_err(|_| SpritePngError::FileNotFound(sprite_path.clone()))?;

        // 2. Parse sprite file
        let sprite = parse_spr(&sprite_data)?;

        // 3. Load and parse ACT file
        let act_path = Self::normalize_path(&request.get_act_path());
        let act_data = self
            .asset_manager
            .load(&act_path)
            .map_err(|_| SpritePngError::FileNotFound(act_path.clone()))?;
        let act = parse_act(&act_data)?;

        // 4. Load optional custom palette
        let custom_palette = if let Some(ref palette_path) = request.palette_path {
            let palette_path = Self::normalize_path(palette_path);
            let palette_data = self
                .asset_manager
                .load(&palette_path)
                .map_err(|_| SpritePngError::FileNotFound(palette_path.clone()))?;
            Some(Self::parse_palette(&palette_data)?)
        } else {
            None
        };

        // 5. Extract the specific frame from ACT
        let (sprite_frame, width, height, offset_x, offset_y) =
            self.extract_frame(&sprite, &act, request)?;

        // 6. Convert to RGBA
        let rgba_data = convert_sprite_frame_to_rgba(
            &sprite_frame,
            sprite.palette.as_ref(),
            custom_palette.as_ref(),
        );

        // 7. Create image
        let image = RgbaImage::from_raw(width as u32, height as u32, rgba_data)
            .ok_or(SpritePngError::ImageCreationFailed)?;

        // 8. Scale if needed
        let final_image = if (request.scale - 1.0).abs() > f32::EPSILON {
            // Validate scale parameter before use
            if request.scale <= 0.0 || !request.scale.is_finite() {
                return Err(SpritePngError::EncodingError(
                    "Scale must be a positive finite number".to_string(),
                ));
            }
            // Limit scale to prevent excessive memory usage
            if request.scale > 16.0 {
                return Err(SpritePngError::EncodingError(
                    "Scale too large (maximum: 16.0)".to_string(),
                ));
            }
            Self::scale_image(&image, request.scale)
        } else {
            image
        };

        // 9. Encode to PNG
        let png_data = Self::encode_to_png(&final_image)?;

        Ok(SpritePngResponse::new(
            png_data,
            final_image.width(),
            final_image.height(),
            offset_x,
            offset_y,
            false, // Fresh generation, not from cache
        ))
    }

    /// Extract a specific frame from sprite and ACT data
    fn extract_frame(
        &self,
        sprite: &crate::infrastructure::ro_formats::RoSprite,
        act: &crate::infrastructure::ro_formats::RoAction,
        request: &SpritePngRequest,
    ) -> Result<
        (
            crate::infrastructure::ro_formats::sprite::SpriteFrame,
            u16,
            u16,
            i32,
            i32,
        ),
        SpritePngError,
    > {
        // Validate action index
        if request.action_index >= act.actions.len() {
            return Err(SpritePngError::InvalidAction(request.action_index));
        }

        let action = &act.actions[request.action_index];

        // Validate frame index
        if request.frame_index >= action.animations.len() {
            return Err(SpritePngError::InvalidFrame(request.frame_index));
        }

        let animation = &action.animations[request.frame_index];

        // Get first layer (main sprite)
        if animation.layers.is_empty() {
            return Err(SpritePngError::NoLayers);
        }

        let layer = &animation.layers[0];

        // Handle negative sprite indices (use index 0 as fallback)
        // RO uses -1 to indicate "no sprite" or invisible layers
        let sprite_index = if layer.sprite_index < 0 {
            0
        } else {
            layer.sprite_index as usize
        };

        // Validate sprite index
        if sprite_index >= sprite.frames.len() {
            return Err(SpritePngError::InvalidSpriteIndex(sprite_index));
        }

        let sprite_frame = sprite.frames[sprite_index].clone();
        let width = sprite_frame.width;
        let height = sprite_frame.height;

        // Get ACT offsets for positioning
        let offset_x = layer.pos[0];
        let offset_y = layer.pos[1];

        Ok((sprite_frame, width, height, offset_x, offset_y))
    }

    /// Parse a 1024-byte palette file to Palette struct
    fn parse_palette(
        data: &[u8],
    ) -> Result<crate::infrastructure::assets::loaders::RoPaletteAsset, SpritePngError> {
        if data.len() != 1024 {
            return Err(SpritePngError::InvalidPalette);
        }

        let mut colors = Vec::with_capacity(256);
        for chunk in data.chunks(4) {
            if chunk.len() >= 4 {
                colors.push([chunk[0], chunk[1], chunk[2], chunk[3]]);
            }
        }

        Ok(crate::infrastructure::assets::loaders::RoPaletteAsset { colors })
    }

    /// Scale an image using nearest neighbor (best for pixel art)
    fn scale_image(image: &RgbaImage, scale: f32) -> RgbaImage {
        let new_width = (image.width() as f32 * scale) as u32;
        let new_height = (image.height() as f32 * scale) as u32;

        image::imageops::resize(image, new_width, new_height, FilterType::Nearest)
    }

    /// Encode an RGBA image to PNG format
    fn encode_to_png(image: &RgbaImage) -> Result<Vec<u8>, SpritePngError> {
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);

        image
            .write_to(&mut cursor, ImageFormat::Png)
            .map_err(|e| SpritePngError::EncodingError(e.to_string()))?;

        Ok(buffer)
    }

    /// Normalize path by converting backslashes to forward slashes
    /// RO uses Windows-style paths but HierarchicalAssetManager expects Unix-style
    fn normalize_path(path: &str) -> String {
        path.replace('\\', "/")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path() {
        let windows_path = r"data\sprite\몬스터\포링.spr";
        let normalized = SpriteRenderer::normalize_path(windows_path);
        assert_eq!(normalized, "data/sprite/몬스터/포링.spr");
    }

    #[test]
    fn test_normalize_path_already_normalized() {
        let unix_path = "data/sprite/monster/poring.spr";
        let normalized = SpriteRenderer::normalize_path(unix_path);
        assert_eq!(normalized, unix_path);
    }

    #[test]
    fn test_parse_palette_invalid_size() {
        let data = vec![0u8; 512]; // Wrong size
        let result = SpriteRenderer::parse_palette(&data);
        assert!(matches!(result, Err(SpritePngError::InvalidPalette)));
    }

    #[test]
    fn test_parse_palette_valid() {
        let data = vec![0u8; 1024]; // Correct size
        let result = SpriteRenderer::parse_palette(&data);
        assert!(result.is_ok());
        let palette = result.unwrap();
        assert_eq!(palette.colors.len(), 256);
    }

    #[test]
    fn test_scale_image() {
        let image = RgbaImage::from_pixel(10, 10, image::Rgba([255, 0, 0, 255]));
        let scaled = SpriteRenderer::scale_image(&image, 2.0);
        assert_eq!(scaled.width(), 20);
        assert_eq!(scaled.height(), 20);
    }

    #[test]
    fn test_encode_to_png() {
        let image = RgbaImage::from_pixel(10, 10, image::Rgba([255, 0, 0, 255]));
        let result = SpriteRenderer::encode_to_png(&image);
        assert!(result.is_ok());

        let png_data = result.unwrap();
        assert!(!png_data.is_empty());

        // Check PNG signature
        assert_eq!(&png_data[0..4], &[0x89, 0x50, 0x4E, 0x47]);
    }
}
