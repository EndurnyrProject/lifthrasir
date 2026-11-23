use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use lifthrasir_client::core::state::GameState;

/// Loaded through the `ro://` composite source. Paths are joined onto the data
/// folder root (`assets/data`), so the bare filename maps to `assets/data/main_bg.jpg`.
const BACKGROUND_IMAGE: &str = "ro://main_bg.jpg";
/// Box-filter downsample factor applied before blurring; the linear sampler
/// upscales the small blurred image smoothly on display.
const DOWNSCALE: usize = 4;
/// Gaussian standard deviation in downsampled pixels (`DOWNSCALE` source pixels each).
const SIGMA: f32 = 3.0;

pub struct MenuBackgroundPlugin;

impl Plugin for MenuBackgroundPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_menu_background);
        app.add_systems(Update, (blur_loaded_background, toggle_on_transition));
    }
}

/// Full-screen images behind the menu screens. The menu roots have transparent
/// backgrounds so one of these shows through.
#[derive(Component)]
struct SharpMenuBackground;

#[derive(Component)]
struct BlurredMenuBackground;

/// The sharp source image, blurred once on the CPU when it finishes loading.
#[derive(Resource)]
struct MenuBackgroundImage(Handle<Image>);

fn background_node() -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: percent(100),
        height: percent(100),
        ..default()
    }
}

fn spawn_menu_background(mut commands: Commands, asset_server: Res<AssetServer>) {
    let image = asset_server.load(BACKGROUND_IMAGE);
    commands.spawn((
        background_node(),
        ImageNode::new(image.clone()),
        GlobalZIndex(i32::MIN),
        Visibility::Hidden,
        Pickable::IGNORE,
        SharpMenuBackground,
    ));
    commands.spawn((
        background_node(),
        ImageNode::default(),
        GlobalZIndex(i32::MIN),
        Visibility::Hidden,
        Pickable::IGNORE,
        BlurredMenuBackground,
    ));
    commands.insert_resource(MenuBackgroundImage(image));
}

fn blur_loaded_background(
    mut events: MessageReader<AssetEvent<Image>>,
    source: Res<MenuBackgroundImage>,
    mut images: ResMut<Assets<Image>>,
    mut blurred: Single<&mut ImageNode, With<BlurredMenuBackground>>,
) -> Result {
    if !events
        .read()
        .any(|event| event.is_loaded_with_dependencies(&source.0))
    {
        return Ok(());
    }
    let image = images
        .get(&source.0)
        .ok_or("menu background image is not loaded")?;
    let blurred_image = blur_image(image)?;
    blurred.image = images.add(blurred_image);
    Ok(())
}

fn blur_image(image: &Image) -> Result<Image> {
    let format = image.texture_descriptor.format;
    if !matches!(
        format,
        TextureFormat::Rgba8UnormSrgb | TextureFormat::Rgba8Unorm
    ) {
        return Err(format!("menu background must be RGBA8, got {format:?}").into());
    }
    let data = image
        .data
        .as_deref()
        .ok_or("menu background has no CPU pixel data")?;
    let (width, height, pixels) = blur_rgba8(data, image.width() as usize, image.height() as usize);
    Ok(Image::new(
        Extent3d {
            width: width as u32,
            height: height as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        format,
        RenderAssetUsages::RENDER_WORLD,
    ))
}

/// Downsamples RGBA8 pixels by [`DOWNSCALE`] and applies a separable Gaussian
/// blur. Returns the downsampled width, height and pixels.
fn blur_rgba8(data: &[u8], width: usize, height: usize) -> (usize, usize, Vec<u8>) {
    let (w, h) = (width / DOWNSCALE, height / DOWNSCALE);
    let kernel = gaussian_kernel();
    let small = downsample(data, width, w, h);
    let horizontal = blur_axis(&small, w, h, &kernel, true);
    let blurred = blur_axis(&horizontal, w, h, &kernel, false);
    (
        w,
        h,
        blurred.iter().map(|value| value.round() as u8).collect(),
    )
}

fn gaussian_kernel() -> Vec<f32> {
    let radius = (3.0 * SIGMA).ceil() as i32;
    let weights: Vec<f32> = (-radius..=radius)
        .map(|i| (-(i * i) as f32 / (2.0 * SIGMA * SIGMA)).exp())
        .collect();
    let total: f32 = weights.iter().sum();
    weights.iter().map(|weight| weight / total).collect()
}

/// Averages each `DOWNSCALE x DOWNSCALE` block into one output pixel.
fn downsample(data: &[u8], width: usize, w: usize, h: usize) -> Vec<f32> {
    let area = (DOWNSCALE * DOWNSCALE) as f32;
    let mut out = vec![0.0; w * h * 4];
    for y in 0..h * DOWNSCALE {
        for x in 0..w * DOWNSCALE {
            let src = (y * width + x) * 4;
            let dst = ((y / DOWNSCALE) * w + x / DOWNSCALE) * 4;
            for c in 0..4 {
                out[dst + c] += f32::from(data[src + c]) / area;
            }
        }
    }
    out
}

/// One 1D Gaussian pass along rows (`horizontal`) or columns, clamping at edges.
fn blur_axis(src: &[f32], w: usize, h: usize, kernel: &[f32], horizontal: bool) -> Vec<f32> {
    let radius = (kernel.len() / 2) as isize;
    let clamp = |value: usize, offset: isize, len: usize| {
        (value as isize + offset).clamp(0, len as isize - 1) as usize
    };
    let mut out = vec![0.0; src.len()];
    for y in 0..h {
        for x in 0..w {
            let dst = (y * w + x) * 4;
            for (k, weight) in kernel.iter().enumerate() {
                let offset = k as isize - radius;
                let (sx, sy) = match horizontal {
                    true => (clamp(x, offset, w), y),
                    false => (x, clamp(y, offset, h)),
                };
                let s = (sy * w + sx) * 4;
                for c in 0..4 {
                    out[dst + c] += src[s + c] * weight;
                }
            }
        }
    }
    out
}

fn toggle_on_transition(
    mut transitions: MessageReader<StateTransitionEvent<GameState>>,
    mut sharp: Single<&mut Visibility, (With<SharpMenuBackground>, Without<BlurredMenuBackground>)>,
    mut blurred: Single<
        &mut Visibility,
        (With<BlurredMenuBackground>, Without<SharpMenuBackground>),
    >,
) {
    let Some(entered) = transitions
        .read()
        .last()
        .and_then(|event| event.entered.as_ref())
    else {
        return;
    };
    let (sharp_visibility, blurred_visibility) = match entered {
        GameState::Login => (Visibility::Visible, Visibility::Hidden),
        GameState::ServerSelection
        | GameState::CharacterSelection
        | GameState::CharacterCreation => (Visibility::Hidden, Visibility::Visible),
        _ => (Visibility::Hidden, Visibility::Hidden),
    };
    **sharp = sharp_visibility;
    **blurred = blurred_visibility;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_preserves_uniform_color_and_downsamples() {
        let pixel = [200, 100, 50, 255];
        let data = pixel.repeat(16 * 8);

        let (width, height, pixels) = blur_rgba8(&data, 16, 8);

        assert_eq!((width, height), (4, 2));
        assert_eq!(pixels, pixel.repeat(4 * 2));
    }

    #[test]
    fn blur_spreads_a_hard_edge_into_a_gradient() {
        let data: Vec<u8> = (0..64 * 4)
            .flat_map(|i| match i % 64 < 32 {
                true => [0, 0, 0, 255],
                false => [255, 255, 255, 255],
            })
            .collect();

        let (width, _, pixels) = blur_rgba8(&data, 64, 4);
        let row: Vec<u8> = pixels[..width * 4].chunks(4).map(|p| p[0]).collect();

        assert!(row.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(row[width / 2 - 1] > 0 && row[width / 2] < 255);
    }
}
