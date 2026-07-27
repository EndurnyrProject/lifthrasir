use crate::grf_vfs::GrfVfs;
use anyhow::{Context, bail};
use image::{ImageFormat, RgbaImage};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;

/// Runtime magenta-key thresholds, mirrored from
/// `game-engine/src/infrastructure/assets/converters.rs::apply_magenta_transparency`.
const MAGENTA_THRESHOLD: u8 = 240;
const GREEN_THRESHOLD: u8 = 15;

/// A GND texture name mapped to the alpha PNG written for it, relative to the
/// map's output directory (e.g. `tex/grass01.png`).
#[derive(Debug, Clone)]
pub struct TextureOut {
    pub source_name: String,
    pub relative_path: String,
}

/// Reads every GND-referenced texture through `vfs`, keys out magenta to real
/// alpha, and writes `tex/<sanitized-name>.png` under `out_dir`.
pub fn normalize_textures(
    vfs: &GrfVfs,
    texture_names: &[String],
    out_dir: &Path,
) -> anyhow::Result<Vec<TextureOut>> {
    let tex_dir = out_dir.join("tex");
    std::fs::create_dir_all(&tex_dir).with_context(|| format!("creating {}", tex_dir.display()))?;

    let sanitized_names = assign_unique_sanitized_names(texture_names)?;

    texture_names
        .iter()
        .zip(sanitized_names)
        .map(|(name, sanitized)| normalize_one(vfs, name, &sanitized, &tex_dir))
        .collect()
}

/// Sanitizes every texture name up front and fails loudly if two distinct
/// source names collide on the same sanitized output filename, so a
/// collision can never silently overwrite an already-written PNG.
fn assign_unique_sanitized_names(texture_names: &[String]) -> anyhow::Result<Vec<String>> {
    let mut seen: HashMap<String, &str> = HashMap::new();
    let mut sanitized_names = Vec::with_capacity(texture_names.len());

    for name in texture_names {
        let sanitized = sanitize_name(name);
        if let Some(previous) = seen.insert(sanitized.clone(), name) {
            bail!(
                "texture name collision: '{name}' and '{previous}' both sanitize to '{sanitized}.png'"
            );
        }
        sanitized_names.push(sanitized);
    }

    Ok(sanitized_names)
}

fn normalize_one(
    vfs: &GrfVfs,
    name: &str,
    sanitized: &str,
    tex_dir: &Path,
) -> anyhow::Result<TextureOut> {
    let logical_path = format!("data/texture/{name}");
    let bmp_bytes = vfs
        .read(&logical_path)
        .with_context(|| format!("texture not found in GRFs: {logical_path}"))?;
    let png_bytes = bmp_bytes_to_keyed_png(&bmp_bytes)
        .with_context(|| format!("converting texture: {logical_path}"))?;

    let file_name = format!("{sanitized}.png");
    let dest = tex_dir.join(&file_name);
    std::fs::write(&dest, &png_bytes).with_context(|| format!("writing {}", dest.display()))?;

    Ok(TextureOut {
        source_name: name.to_string(),
        relative_path: format!("tex/{file_name}"),
    })
}

fn bmp_bytes_to_keyed_png(bmp_bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut image = image::load_from_memory_with_format(bmp_bytes, ImageFormat::Bmp)
        .context("decoding BMP")?
        .to_rgba8();
    apply_magenta_transparency(&mut image);

    let mut png_bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png)
        .context("encoding PNG")?;
    Ok(png_bytes)
}

/// Zeroes RGB and alpha on magenta-keyed pixels, alpha 255 everywhere else.
/// NOTE: the unconditional 255 assumes BMP decode always yields opaque alpha
/// (true for the 8-bit/24-bit BMPs the GRFs ship); source alpha is discarded.
fn apply_magenta_transparency(image: &mut RgbaImage) {
    for pixel in image.pixels_mut() {
        let [r, g, b, _] = pixel.0;
        let is_keyed = r >= MAGENTA_THRESHOLD && g <= GREEN_THRESHOLD && b >= MAGENTA_THRESHOLD;
        pixel.0 = if is_keyed {
            [0, 0, 0, 0]
        } else {
            [r, g, b, 255]
        };
    }
}

/// Lowercase ASCII, path-safe filename for a GND texture name. Keeps the
/// whole relative path (directory components included) flattened into one
/// component, so distinct source paths with the same basename (e.g.
/// `floor\dirt.bmp` vs `wall\dirt.bmp`) never sanitize to the same output.
fn sanitize_name(name: &str) -> String {
    let stem = name.rsplit_once('.').map_or(name, |(base, _)| base);

    let sanitized: String = stem
        .chars()
        .map(|c| {
            let lower = c.to_ascii_lowercase();
            if lower.is_ascii_alphanumeric() || lower == '_' || lower == '-' {
                lower
            } else {
                '_'
            }
        })
        .collect();

    if sanitized.is_empty() {
        "texture".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GrfEntry;
    use image::Rgba;

    fn encode_bmp(pixels: &[[u8; 4]], width: u32, height: u32) -> Vec<u8> {
        let mut image = RgbaImage::new(width, height);
        for (i, pixel) in pixels.iter().enumerate() {
            image.put_pixel(i as u32 % width, i as u32 / width, Rgba(*pixel));
        }
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Bmp)
            .expect("encode synthetic bmp");
        bytes
    }

    #[test]
    fn magenta_pixels_become_transparent_others_stay_opaque() {
        let magenta = [255, 0, 255, 255];
        let green = [10, 200, 10, 255];
        let bmp = encode_bmp(&[magenta, green], 2, 1);

        let png_bytes = bmp_bytes_to_keyed_png(&bmp).expect("convert");
        let decoded = image::load_from_memory_with_format(&png_bytes, ImageFormat::Png)
            .expect("decode png")
            .to_rgba8();

        assert_eq!(decoded.get_pixel(0, 0).0, [0, 0, 0, 0]);
        assert_eq!(decoded.get_pixel(1, 0).0, [10, 200, 10, 255]);
    }

    #[test]
    fn near_magenta_within_threshold_is_also_keyed() {
        let near_magenta = [245, 10, 250, 255];
        let bmp = encode_bmp(&[near_magenta], 1, 1);

        let png_bytes = bmp_bytes_to_keyed_png(&bmp).expect("convert");
        let decoded = image::load_from_memory_with_format(&png_bytes, ImageFormat::Png)
            .expect("decode png")
            .to_rgba8();

        assert_eq!(decoded.get_pixel(0, 0).0, [0, 0, 0, 0]);
    }

    #[test]
    fn missing_texture_in_vfs_fails_loudly() {
        let vfs = GrfVfs::open(&[] as &[&GrfEntry]).expect("empty vfs opens");
        let out = tempfile::tempdir().expect("tempdir");

        let err = normalize_textures(&vfs, &["grass01.bmp".to_string()], out.path())
            .expect_err("missing texture must fail");

        assert!(
            err.to_string().contains("grass01.bmp"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn sanitize_name_flattens_directories_strips_extension_and_non_ascii() {
        assert_eq!(sanitize_name("grass01.bmp"), "grass01");
        assert_eq!(sanitize_name("sub\\dir\\Grass 01.bmp"), "sub_dir_grass_01");
    }

    #[test]
    fn distinct_subdirs_same_basename_produce_distinct_sanitized_names() {
        let names = vec!["floor\\dirt.bmp".to_string(), "wall\\dirt.bmp".to_string()];

        let sanitized = assign_unique_sanitized_names(&names).expect("no collision");

        assert_ne!(sanitized[0], sanitized[1]);
        assert_eq!(sanitized[0], "floor_dirt");
        assert_eq!(sanitized[1], "wall_dirt");
    }

    #[test]
    fn sanitize_collision_across_distinct_sources_bails() {
        let names = vec!["a b.bmp".to_string(), "a_b.bmp".to_string()];

        let err = assign_unique_sanitized_names(&names).expect_err("must collide");

        let message = err.to_string();
        assert!(message.contains("a b.bmp"), "unexpected error: {message}");
        assert!(message.contains("a_b.bmp"), "unexpected error: {message}");
    }
}
