use super::{
    error::SpritePngError,
    types::{SpritePngRequest, SpritePngResponse},
};
use crate::infrastructure::assets::{
    converters::convert_sprite_frame_to_rgba,
    loaders::{RoActAsset, RoPaletteAsset, RoSpriteAsset},
};
use bevy::{
    asset::{AssetServer, Assets, Handle, LoadState},
    prelude::*,
};
use image::{imageops::FilterType, ImageFormat, RgbaImage};
use std::{
    io::Cursor,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
    time::Duration,
};

type SpriteRenderRequest = (
    SpritePngRequest,
    Sender<Result<SpritePngResponse, SpritePngError>>,
);
type SpriteRenderReceiver = Arc<Mutex<Receiver<SpriteRenderRequest>>>;

/// Component to track sprite render requests waiting for assets to load
#[derive(Component)]
struct PendingSpritePngRequest {
    request: SpritePngRequest,
    sprite_handle: Handle<RoSpriteAsset>,
    act_handle: Handle<RoActAsset>,
    palette_handle: Option<Handle<RoPaletteAsset>>,
    response_tx: Sender<Result<SpritePngResponse, SpritePngError>>,
}

/// Sprite renderer that uses Bevy's asset system to load RO assets
/// Communicates with Bevy's ECS through a channel-based request/response system
pub struct SpriteRenderer {
    request_tx: Sender<SpriteRenderRequest>,
}

impl SpriteRenderer {
    /// Create a new sprite renderer that communicates with Bevy's asset system
    pub fn new(request_tx: Sender<SpriteRenderRequest>) -> Self {
        Self { request_tx }
    }

    /// Create a sprite renderer integrated with Bevy's App
    /// This should be called during app setup to register the rendering system
    pub fn create_with_app(app: &mut App) -> Self {
        let (request_tx, request_rx) = channel::<SpriteRenderRequest>();

        // Store the receiver as a resource
        app.insert_resource(SpriteRenderRequestReceiver(Arc::new(Mutex::new(
            request_rx,
        ))));

        // Add the systems that handle sprite render requests
        // 1. Receive requests and start loading assets
        // 2. Poll each frame and render when assets are ready
        app.add_systems(
            Update,
            (
                receive_sprite_render_requests,
                process_loaded_sprite_requests,
            )
                .chain(),
        );

        Self::new(request_tx)
    }

    /// Render a sprite frame to PNG format (now sends request to Bevy system)
    pub fn render_to_png(
        &self,
        request: &SpritePngRequest,
    ) -> Result<SpritePngResponse, SpritePngError> {
        let (response_tx, response_rx) = channel();

        self.request_tx
            .send((request.clone(), response_tx))
            .map_err(|_| SpritePngError::EncodingError("Failed to send request".to_string()))?;

        // Wait for response with timeout
        response_rx
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| {
                SpritePngError::EncodingError("Timeout waiting for sprite rendering".to_string())
            })?
    }

    /// Normalize path by converting backslashes to forward slashes
    fn normalize_path(path: &str) -> String {
        path.replace('\\', "/")
    }
}

#[derive(Resource)]
struct SpriteRenderRequestReceiver(SpriteRenderReceiver);

/// Bevy system that receives sprite render requests and starts loading assets
fn receive_sprite_render_requests(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    receiver: Res<SpriteRenderRequestReceiver>,
) {
    let Ok(rx) = receiver.0.try_lock() else {
        return;
    };

    // Process all pending requests (non-blocking)
    while let Ok((request, response_tx)) = rx.try_recv() {
        // Normalize paths
        let sprite_path = SpriteRenderer::normalize_path(&request.sprite_path);
        let act_path = SpriteRenderer::normalize_path(&request.get_act_path());

        // Start loading assets through AssetServer (async)
        let sprite_handle: Handle<RoSpriteAsset> = asset_server.load(&sprite_path);
        let act_handle: Handle<RoActAsset> = asset_server.load(&act_path);
        let palette_handle = request
            .palette_path
            .as_ref()
            .map(|p| asset_server.load::<RoPaletteAsset>(SpriteRenderer::normalize_path(p)));

        // Spawn entity to track this pending request
        commands.spawn(PendingSpritePngRequest {
            request,
            sprite_handle,
            act_handle,
            palette_handle,
            response_tx,
        });
    }
}

/// Bevy system that polls pending requests and renders when assets are loaded
fn process_loaded_sprite_requests(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    sprite_assets: Res<Assets<RoSpriteAsset>>,
    act_assets: Res<Assets<RoActAsset>>,
    palette_assets: Res<Assets<RoPaletteAsset>>,
    pending_query: Query<(Entity, &PendingSpritePngRequest)>,
) {
    for (entity, pending) in pending_query.iter() {
        // Check if all required assets are loaded
        let sprite_state = asset_server.load_state(&pending.sprite_handle);
        let act_state = asset_server.load_state(&pending.act_handle);

        // Check for failures first
        if matches!(sprite_state, LoadState::Failed(_)) {
            let sprite_path = SpriteRenderer::normalize_path(&pending.request.sprite_path);
            let _ = pending
                .response_tx
                .send(Err(SpritePngError::FileNotFound(sprite_path)));
            commands.entity(entity).despawn();
            continue;
        }

        if matches!(act_state, LoadState::Failed(_)) {
            let act_path = SpriteRenderer::normalize_path(&pending.request.get_act_path());
            let _ = pending
                .response_tx
                .send(Err(SpritePngError::FileNotFound(act_path)));
            commands.entity(entity).despawn();
            continue;
        }

        // Check palette if present
        if let Some(ref palette_handle) = pending.palette_handle {
            let palette_state = asset_server.load_state(palette_handle);
            if matches!(palette_state, LoadState::Failed(_)) {
                let palette_path = pending.request.palette_path.as_ref().unwrap().clone();
                let _ = pending
                    .response_tx
                    .send(Err(SpritePngError::FileNotFound(palette_path)));
                commands.entity(entity).despawn();
                continue;
            }
            // Still loading - wait for next frame
            if !matches!(palette_state, LoadState::Loaded) {
                continue;
            }
        }

        // Wait for assets to finish loading
        if !matches!(sprite_state, LoadState::Loaded) || !matches!(act_state, LoadState::Loaded) {
            continue; // Still loading - try again next frame
        }

        // All assets loaded - get them from asset storage
        let Some(sprite) = sprite_assets.get(&pending.sprite_handle) else {
            continue; // Shouldn't happen but be safe
        };
        let Some(act) = act_assets.get(&pending.act_handle) else {
            continue;
        };
        let custom_palette = pending
            .palette_handle
            .as_ref()
            .and_then(|h| palette_assets.get(h));

        // Render the sprite with loaded assets
        let result =
            render_sprite_with_loaded_assets(&pending.request, sprite, act, custom_palette);

        // Send response back to Tauri
        let _ = pending.response_tx.send(result);

        // Clean up the pending request entity
        commands.entity(entity).despawn();
    }
}

/// Helper function to render sprite using already-loaded assets
fn render_sprite_with_loaded_assets(
    request: &SpritePngRequest,
    sprite_asset: &RoSpriteAsset,
    act_asset: &RoActAsset,
    custom_palette: Option<&RoPaletteAsset>,
) -> Result<SpritePngResponse, SpritePngError> {
    // Extract frame
    let (sprite_frame, width, height, offset_x, offset_y) =
        extract_frame(&sprite_asset.sprite, &act_asset.action, request)?;

    // Convert to RGBA
    let rgba_data = convert_sprite_frame_to_rgba(
        &sprite_frame,
        sprite_asset.sprite.palette.as_ref(),
        custom_palette,
    );

    // Create image
    let image = RgbaImage::from_raw(width as u32, height as u32, rgba_data)
        .ok_or(SpritePngError::ImageCreationFailed)?;

    // Scale if needed
    let final_image = if (request.scale - 1.0).abs() > f32::EPSILON {
        if request.scale <= 0.0 || !request.scale.is_finite() {
            return Err(SpritePngError::EncodingError(
                "Scale must be a positive finite number".to_string(),
            ));
        }
        if request.scale > 16.0 {
            return Err(SpritePngError::EncodingError(
                "Scale too large (maximum: 16.0)".to_string(),
            ));
        }
        scale_image(&image, request.scale)
    } else {
        image
    };

    // Encode to PNG
    let png_data = encode_to_png(&final_image)?;

    Ok(SpritePngResponse::new(
        png_data,
        final_image.width(),
        final_image.height(),
        offset_x,
        offset_y,
        false,
    ))
}

/// Extract a specific frame from sprite and ACT data
fn extract_frame(
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

    // Handle negative sprite indices
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
    fn test_scale_image() {
        let image = RgbaImage::from_pixel(10, 10, image::Rgba([255, 0, 0, 255]));
        let scaled = scale_image(&image, 2.0);
        assert_eq!(scaled.width(), 20);
        assert_eq!(scaled.height(), 20);
    }

    #[test]
    fn test_encode_to_png() {
        let image = RgbaImage::from_pixel(10, 10, image::Rgba([255, 0, 0, 255]));
        let result = encode_to_png(&image);
        assert!(result.is_ok());

        let png_data = result.unwrap();
        assert!(!png_data.is_empty());

        // Check PNG signature
        assert_eq!(&png_data[0..4], &[0x89, 0x50, 0x4E, 0x47]);
    }
}
