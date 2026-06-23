use bevy::{
    asset::{io::Reader, AssetLoader, LoadContext, RenderAssetUsages},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Default, TypePath)]
pub struct SvgLoader;

#[derive(Serialize, Deserialize, Clone)]
pub struct SvgSettings {
    /// Oversample factor applied to the SVG's intrinsic size before rasterizing.
    /// Higher values keep icons crisp when the UI scales them down.
    pub scale: f32,
}

impl Default for SvgSettings {
    fn default() -> Self {
        Self { scale: 2.0 }
    }
}

#[derive(Debug, Error)]
pub enum SvgLoaderError {
    #[error("Could not load SVG: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not parse SVG: {0}")]
    Parse(#[from] resvg::usvg::Error),
    #[error("Invalid SVG dimensions")]
    InvalidDimensions,
    #[error("Could not allocate pixmap")]
    Pixmap,
}

impl AssetLoader for SvgLoader {
    type Asset = Image;
    type Settings = SvgSettings;
    type Error = SvgLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let tree = resvg::usvg::Tree::from_data(&bytes, &resvg::usvg::Options::default())?;

        let size = tree.size();
        let width = (size.width() * settings.scale).ceil() as u32;
        let height = (size.height() * settings.scale).ceil() as u32;

        if width == 0 || height == 0 {
            return Err(SvgLoaderError::InvalidDimensions);
        }

        let mut pixmap =
            resvg::tiny_skia::Pixmap::new(width, height).ok_or(SvgLoaderError::Pixmap)?;

        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(settings.scale, settings.scale),
            &mut pixmap.as_mut(),
        );

        // tiny-skia emits premultiplied alpha; Bevy UI expects straight alpha.
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for pixel in pixmap.pixels() {
            let color = pixel.demultiply();
            rgba.extend_from_slice(&[color.red(), color.green(), color.blue(), color.alpha()]);
        }

        Ok(Image::new(
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            rgba,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        ))
    }

    fn extensions(&self) -> &[&str] {
        &["svg"]
    }
}
