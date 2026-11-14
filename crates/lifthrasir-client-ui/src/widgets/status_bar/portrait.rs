//! The hero's portrait: the head sprite and the headgear worn on it, facing the viewer.
//!
//! The pieces are blended on the CPU into one image because `bevy_ui` clips child
//! nodes to rectangles only, while a single `ImageNode` follows its own
//! `border_radius`, which is what makes the portrait round.

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use lifthrasir_assets::animation::asset::{FramePart, RoAnimationAsset};
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_runtime::actors::EquipmentSlot;
use lifthrasir_runtime::actors::sprites::components::PlayerAppearance;

/// RO pixels across the portrait: a head with room for a hat above it.
const SPAN: f32 = 56.0;
/// How far below the portrait's centre the head's centre sits, in RO pixels.
const HEAD_DROP: f32 = 6.0;
/// Headgear from the back layer to the front one.
const HEADGEAR: [EquipmentSlot; 3] = [
    EquipmentSlot::HeadBottom,
    EquipmentSlot::HeadMid,
    EquipmentSlot::HeadTop,
];

/// The node the portrait is drawn on. Any in-game widget may mount one.
#[derive(Component, Default, Clone)]
pub(crate) struct Portrait;

/// A sprite frame to blend: its RGBA pixels, and where it lands on the canvas in
/// canvas pixels (y down).
struct Layer<'a> {
    pixels: &'a [u8],
    size: UVec2,
    min: Vec2,
    max: Vec2,
    mirror: bool,
}

/// Repaints the portrait when the hero's head or headgear changes, and for a frame
/// spawned after that.
pub(super) fn paint_portrait(
    mut commands: Commands,
    hero: Query<Ref<PlayerAppearance>, With<LocalPlayer>>,
    animations: Res<Assets<RoAnimationAsset>>,
    mut images: ResMut<Assets<Image>>,
    frames: Query<(Entity, Ref<Portrait>)>,
) {
    let Ok(appearance) = hero.single() else {
        return;
    };
    if !appearance.is_changed() && !frames.iter().any(|(_, frame)| frame.is_added()) {
        return;
    }
    let Some(image) = paint(&appearance, &animations, &images) else {
        return;
    };
    let handle = images.add(image);
    for (frame, _) in &frames {
        commands
            .entity(frame)
            .insert(ImageNode::new(handle.clone()));
    }
}

/// The first part of the south-facing idle frame, with the frame's attach point.
fn facing_viewer(animation: &RoAnimationAsset) -> Option<(&FramePart, Option<Vec2>)> {
    let frame = animation.actions.first()?.frames.first()?;
    Some((frame.parts.first()?, frame.attach_point))
}

fn paint(
    appearance: &PlayerAppearance,
    animations: &Assets<RoAnimationAsset>,
    images: &Assets<Image>,
) -> Option<Image> {
    let pixels = |animation: &RoAnimationAsset, part: &FramePart| {
        let image = images.get(animation.textures.get(part.texture_index)?)?;
        let rgba = image.texture_descriptor.format == TextureFormat::Rgba8UnormSrgb;
        let data = image.data.as_deref().filter(|_| rgba)?;
        Some((data, image.size()))
    };

    let head = animations.get(&appearance.head)?;
    let (head_part, head_attach) = facing_viewer(head)?;
    let (head_pixels, head_size) = pixels(head, head_part)?;
    // Sprite textures may be upscaled; the canvas keeps the head's texel density.
    let density = head_size.x as f32 / head_part.texture_size.x.max(1.0);
    let side = (SPAN * density).round() as u32;

    let mut canvas = vec![0u8; (side * side * 4) as usize];
    let head_layer = layer(head_part, Vec2::ZERO, density, (head_pixels, head_size));
    blit(&mut canvas, side, &head_layer);
    for slot in HEADGEAR {
        let Some(gear) = appearance
            .equipment
            .get(&slot)
            .and_then(|handle| animations.get(handle))
        else {
            continue;
        };
        let Some((part, attach)) = facing_viewer(gear) else {
            continue;
        };
        let Some(source) = pixels(gear, part) else {
            continue;
        };
        // Headgear rides the head's attach point, as `sync_headgear_layer` places it.
        let neck = match (head_attach, attach) {
            (Some(head), Some(gear)) => head - gear,
            _ => Vec2::ZERO,
        };
        let offset = part.position + neck - head_part.position;
        blit(&mut canvas, side, &layer(part, offset, density, source));
    }

    let mut image = Image::new(
        Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        canvas,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::linear();
    Some(image)
}

/// Where `part` lands on the canvas when its centre is `offset` RO pixels (y up) from
/// the head's centre, at `density` canvas pixels per RO pixel.
fn layer<'a>(
    part: &FramePart,
    offset: Vec2,
    density: f32,
    (pixels, size): (&'a [u8], UVec2),
) -> Layer<'a> {
    let head_centre = Vec2::new(SPAN / 2.0, SPAN / 2.0 + HEAD_DROP);
    let centre = head_centre + Vec2::new(offset.x, -offset.y);
    let half = (part.texture_size * part.scale).abs() / 2.0;
    Layer {
        pixels,
        size,
        min: (centre - half) * density,
        max: (centre + half) * density,
        mirror: part.mirror != (part.scale.x < 0.0),
    }
}

/// Blends `layer` over the square `canvas` of `side` pixels, stretching its pixels
/// over its rectangle and cutting off what falls outside.
fn blit(canvas: &mut [u8], side: u32, layer: &Layer) {
    let extent = layer.max - layer.min;
    let texels = (layer.size.x * layer.size.y * 4) as usize;
    if extent.min_element() <= 0.0 || texels == 0 || layer.pixels.len() < texels {
        return;
    }
    let bound = |edge: f32| edge.round().clamp(0.0, side as f32) as u32;
    for y in bound(layer.min.y)..bound(layer.max.y) {
        for x in bound(layer.min.x)..bound(layer.max.x) {
            let at = (Vec2::new(x as f32, y as f32) + 0.5 - layer.min) / extent;
            let across = if layer.mirror { 1.0 - at.x } else { at.x };
            let texel = (Vec2::new(across, at.y) * layer.size.as_vec2())
                .as_uvec2()
                .min(layer.size - 1);
            let from = ((texel.y * layer.size.x + texel.x) * 4) as usize;
            let onto = ((y * side + x) * 4) as usize;
            over(&mut canvas[onto..onto + 4], &layer.pixels[from..from + 4]);
        }
    }
}

/// Straight-alpha "source over" of one RGBA pixel.
fn over(under: &mut [u8], top: &[u8]) {
    let alpha = u32::from(top[3]);
    if alpha == 255 {
        under.copy_from_slice(top);
        return;
    }
    let rest = u32::from(under[3]) * (255 - alpha) / 255;
    let out = alpha + rest;
    if out == 0 {
        return;
    }
    for channel in 0..3 {
        let mixed = u32::from(top[channel]) * alpha + u32::from(under[channel]) * rest;
        under[channel] = (mixed / out) as u8;
    }
    under[3] = out as u8;
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: [u8; 4] = [255, 0, 0, 255];
    const BLUE: [u8; 4] = [0, 0, 255, 255];

    fn pixel(canvas: &[u8], side: u32, x: u32, y: u32) -> &[u8] {
        let at = ((y * side + x) * 4) as usize;
        &canvas[at..at + 4]
    }

    #[test]
    fn a_layer_lands_on_its_rectangle_and_nowhere_else() {
        let pixels = [RED, BLUE].concat();
        let mut canvas = vec![0u8; 4 * 4 * 4];
        let layer = Layer {
            pixels: &pixels,
            size: UVec2::new(2, 1),
            min: Vec2::new(1.0, 1.0),
            max: Vec2::new(3.0, 2.0),
            mirror: false,
        };

        blit(&mut canvas, 4, &layer);

        assert_eq!(pixel(&canvas, 4, 1, 1), RED);
        assert_eq!(pixel(&canvas, 4, 2, 1), BLUE);
        assert_eq!(pixel(&canvas, 4, 0, 1), [0; 4]);
        assert_eq!(pixel(&canvas, 4, 1, 2), [0; 4]);
    }

    #[test]
    fn a_mirrored_layer_is_flipped_and_clipped_at_the_canvas_edge() {
        let pixels = [RED, BLUE].concat();
        let mut canvas = vec![0u8; 2 * 2 * 4];
        let layer = Layer {
            pixels: &pixels,
            size: UVec2::new(2, 1),
            min: Vec2::new(0.0, 1.0),
            max: Vec2::new(2.0, 5.0),
            mirror: true,
        };

        blit(&mut canvas, 2, &layer);

        assert_eq!(pixel(&canvas, 2, 0, 1), BLUE);
        assert_eq!(pixel(&canvas, 2, 1, 1), RED);
        assert_eq!(pixel(&canvas, 2, 0, 0), [0; 4]);
    }

    #[test]
    fn a_clear_pixel_keeps_what_is_under_it() {
        let mut under = RED;
        over(&mut under, &[0, 0, 255, 0]);
        assert_eq!(under, RED);
    }
}
