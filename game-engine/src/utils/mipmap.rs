use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

/// Generate a full box-filtered mipmap chain for an image in place and switch
/// its sampler to trilinear + anisotropic filtering.
///
/// Anisotropic filtering only does anything when the texture actually has mip
/// levels to select from, so the two are wired together here. The image is
/// converted to RGBA8 first; formats that cannot be expressed as RGBA8 are left
/// untouched (with a warning). Idempotent: an image that already carries a mip
/// chain is returned unchanged.
pub fn generate_mipmaps_with_anisotropy(image: &mut Image, anisotropy: u16) {
    if image.texture_descriptor.mip_level_count > 1 {
        return;
    }

    if !ensure_rgba8(image) {
        warn!(
            "mipmap: unsupported format {:?}, leaving texture unfiltered",
            image.texture_descriptor.format
        );
        return;
    }

    let srgb = image.texture_descriptor.format.is_srgb();
    let lut = srgb.then(srgb_decode_lut);

    let mut width = image.width();
    let mut height = image.height();

    let Some(level0) = image.data.as_ref() else {
        return;
    };

    let mut chain = level0.clone();
    let mut current = level0.clone();
    let mut levels = 1u32;

    while width > 1 || height > 1 {
        let next_w = (width / 2).max(1);
        let next_h = (height / 2).max(1);
        current = downsample_box(&current, width, height, next_w, next_h, lut.as_ref());
        chain.extend_from_slice(&current);
        width = next_w;
        height = next_h;
        levels += 1;
    }

    image.texture_descriptor.mip_level_count = levels;
    image.data = Some(chain);

    apply_anisotropic_sampler(image, anisotropy);
}

/// Switch an image's sampler to trilinear + anisotropic filtering at the given
/// clamp (`1` = plain trilinear, no anisotropy). Cheap enough to re-run when the
/// anisotropy setting changes, without regenerating the mip chain.
pub fn apply_anisotropic_sampler(image: &mut Image, anisotropy: u16) {
    let mut sampler = ImageSamplerDescriptor::linear();
    sampler.set_anisotropic_filter(anisotropy);
    image.sampler = ImageSampler::Descriptor(sampler);
}

fn ensure_rgba8(image: &mut Image) -> bool {
    if matches!(
        image.texture_descriptor.format,
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb
    ) {
        return true;
    }

    match image.convert(TextureFormat::Rgba8UnormSrgb) {
        Some(converted) => {
            *image = converted;
            true
        }
        None => false,
    }
}

/// Average each 2x2 block of source texels into one destination texel. Colour
/// channels are averaged in linear light when `lut` is provided (sRGB sources),
/// alpha is always averaged linearly.
fn downsample_box(
    src: &[u8],
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
    lut: Option<&[f32; 256]>,
) -> Vec<u8> {
    let src_w = src_w as usize;
    let src_h = src_h as usize;
    let dst_w = dst_w as usize;
    let dst_h = dst_h as usize;

    let mut dst = vec![0u8; dst_w * dst_h * 4];

    for y in 0..dst_h {
        let sy0 = (y * 2).min(src_h - 1);
        let sy1 = (y * 2 + 1).min(src_h - 1);
        for x in 0..dst_w {
            let sx0 = (x * 2).min(src_w - 1);
            let sx1 = (x * 2 + 1).min(src_w - 1);

            let mut acc = [0.0f32; 4];
            for (sx, sy) in [(sx0, sy0), (sx1, sy0), (sx0, sy1), (sx1, sy1)] {
                let idx = (sy * src_w + sx) * 4;
                for (c, slot) in acc.iter_mut().enumerate().take(3) {
                    let raw = src[idx + c];
                    *slot += match lut {
                        Some(table) => table[raw as usize],
                        None => raw as f32 / 255.0,
                    };
                }
                acc[3] += src[idx + 3] as f32 / 255.0;
            }

            let didx = (y * dst_w + x) * 4;
            for (c, value) in acc.iter().enumerate().take(3) {
                let avg = value / 4.0;
                dst[didx + c] = if lut.is_some() {
                    linear_to_srgb(avg)
                } else {
                    (avg * 255.0).round().clamp(0.0, 255.0) as u8
                };
            }
            dst[didx + 3] = (acc[3] / 4.0 * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }

    dst
}

fn srgb_decode_lut() -> [f32; 256] {
    let mut lut = [0.0f32; 256];
    for (i, slot) in lut.iter_mut().enumerate() {
        let c = i as f32 / 255.0;
        *slot = if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        };
    }
    lut
}

fn linear_to_srgb(l: f32) -> u8 {
    let c = if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    };
    (c.clamp(0.0, 1.0) * 255.0).round() as u8
}
