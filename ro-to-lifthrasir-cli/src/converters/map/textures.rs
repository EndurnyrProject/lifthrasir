use crate::converters::ktx2_out::encode_ktx2;
use crate::grf_vfs::GrfVfs;
use anyhow::{Context, bail, ensure};
use image::{ImageFormat, RgbaImage};
use std::collections::HashMap;
use std::path::Path;

/// Runtime magenta-key thresholds, mirrored from
/// `game-engine/src/infrastructure/assets/converters.rs::apply_magenta_transparency`.
const MAGENTA_THRESHOLD: u8 = 240;
const GREEN_THRESHOLD: u8 = 15;

/// A GND texture name mapped to the KTX2 written for it, relative to the
/// map's output directory (e.g. `tex/grass01.ktx2`).
#[derive(Debug, Clone)]
pub struct TextureOut {
    pub source_name: String,
    pub relative_path: String,
}

/// Reads every GND-referenced texture through `vfs`, keys out magenta to real
/// alpha, and writes `tex/<sanitized-name>.ktx2` under `out_dir`.
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
/// collision can never silently overwrite an already-written KTX2.
fn assign_unique_sanitized_names(texture_names: &[String]) -> anyhow::Result<Vec<String>> {
    let mut seen: HashMap<String, &str> = HashMap::new();
    let mut sanitized_names = Vec::with_capacity(texture_names.len());

    for name in texture_names {
        let sanitized = sanitize_name(name);
        if let Some(previous) = seen.insert(sanitized.clone(), name) {
            ensure!(
                canonical_name(previous) == canonical_name(name),
                "texture name collision: '{name}' and '{previous}' both sanitize to '{sanitized}.ktx2'"
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

    // Some maps reference a texture that is in none of the archives. The map is
    // otherwise complete, so it gets an obviously wrong stand-in rather than
    // failing the whole conversion - but every substitution is reported, so a
    // missing texture cannot pass unnoticed.
    let ktx2_bytes = match vfs.read(&logical_path) {
        Some(source_bytes) => texture_bytes_to_ktx2(name, &source_bytes)
            .with_context(|| format!("converting texture: {logical_path}"))?,
        None => {
            println!("  missing texture, using a placeholder: {logical_path}");
            crate::converters::model::fallback_texture_ktx2()
                .with_context(|| format!("building placeholder for {logical_path}"))?
        }
    };

    let file_name = format!("{sanitized}.ktx2");
    let dest = tex_dir.join(&file_name);
    std::fs::write(&dest, &ktx2_bytes).with_context(|| format!("writing {}", dest.display()))?;

    Ok(TextureOut {
        source_name: name.to_string(),
        relative_path: format!("tex/{file_name}"),
    })
}

/// Normalizes one GRF texture into RGBA KTX2 the way the runtime loaders would.
///
/// The extension picks the semantics, mirroring the asset-server dispatch:
/// BMPs are magenta-keyed (`bmp_loader.rs`), TGAs carry real 8-bit alpha and
/// are taken verbatim (`tga_loader.rs`). RSM props reference both; GND ground
/// textures are BMP only.
pub fn texture_bytes_to_ktx2(source_name: &str, bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let extension = source_name
        .rsplit_once('.')
        .map_or(String::new(), |(_, ext)| ext.to_ascii_lowercase());

    match extension.as_str() {
        "bmp" => bmp_bytes_to_keyed_ktx2(bytes),
        "tga" => decoded_to_ktx2(
            image::load_from_memory_with_format(bytes, ImageFormat::Tga).context("decoding TGA")?,
        ),
        // Everything else reaches Bevy's stock `ImageLoader` at runtime, which
        // keys nothing: JPEG cannot carry an exact key colour through lossy
        // compression, and PNG carries its own alpha.
        "jpg" | "jpeg" => decoded_to_ktx2(
            image::load_from_memory_with_format(bytes, ImageFormat::Jpeg)
                .context("decoding JPEG")?,
        ),
        "png" => decoded_to_ktx2(
            image::load_from_memory_with_format(bytes, ImageFormat::Png).context("decoding PNG")?,
        ),
        other => bail!("unsupported texture format '{other}' for '{source_name}'"),
    }
}

pub fn bmp_bytes_to_keyed_ktx2(bmp_bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut image = image::load_from_memory_with_format(bmp_bytes, ImageFormat::Bmp)
        .context("decoding BMP")?
        .to_rgba8();
    apply_magenta_transparency(&mut image);

    encode_ktx2(&image, true)
}

fn decoded_to_ktx2(image: image::DynamicImage) -> anyhow::Result<Vec<u8>> {
    encode_ktx2(&image.to_rgba8(), true)
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

/// Case and path separators are folded out, mirroring GRF lookup: a name is
/// resolved case-insensitively with `/` and `\` alike, so two spellings that
/// differ only that way address one file and must pool as one texture.
pub fn canonical_name(name: &str) -> String {
    name.replace('/', "\\").to_ascii_lowercase()
}

/// Lowercase ASCII, path-safe filename for a GRF texture name. Keeps the
/// whole relative path (directory components included) flattened into one
/// component, so distinct source paths with the same basename (e.g.
/// `floor\dirt.bmp` vs `wall\dirt.bmp`) never sanitize to the same output.
/// The extension is part of that name -- retail props reference
/// `izlude\iz_rookie_06.bmp` and `izlude\iz_rookie_06.tga`, two different
/// images -- so it is folded in like any other character rather than stripped.
///
/// Roughly half the GRF texture namespace is Korean; those names lose every
/// character to the ASCII filter, so a short digest of the source name is
/// appended to keep them apart. `assign_unique_sanitized_names` stays the
/// loud backstop.
pub fn sanitize_name(name: &str) -> String {
    let sanitized: String = name
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

    let sanitized = if sanitized.is_empty() {
        "texture".to_string()
    } else {
        sanitized
    };

    if name.is_ascii() {
        return sanitized;
    }
    format!(
        "{sanitized}_{}",
        &blake3::hash(canonical_name(name).as_bytes()).to_hex()[..8]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GrfEntry;
    use image::Rgba;
    use std::io::Cursor;

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

    fn decode_base_level(bytes: &[u8]) -> RgbaImage {
        let reader = ktx2::Reader::new(bytes).expect("parse KTX2");
        let header = reader.header();
        let level = reader.levels().next().expect("base level");
        let pixels = zstd::bulk::decompress(level.data, level.uncompressed_byte_length as usize)
            .expect("decompress base level");
        RgbaImage::from_raw(header.pixel_width, header.pixel_height, pixels).expect("RGBA pixels")
    }

    #[test]
    fn magenta_pixels_become_transparent_others_stay_opaque() {
        let magenta = [255, 0, 255, 255];
        let green = [10, 200, 10, 255];
        let bmp = encode_bmp(&[magenta, green], 2, 1);

        let decoded = decode_base_level(&bmp_bytes_to_keyed_ktx2(&bmp).expect("convert"));

        assert_eq!(decoded.get_pixel(0, 0).0, [0, 0, 0, 0]);
        assert_eq!(decoded.get_pixel(1, 0).0, [10, 200, 10, 255]);
    }

    #[test]
    fn near_magenta_within_threshold_is_also_keyed() {
        let near_magenta = [245, 10, 250, 255];
        let bmp = encode_bmp(&[near_magenta], 1, 1);

        let decoded = decode_base_level(&bmp_bytes_to_keyed_ktx2(&bmp).expect("convert"));

        assert_eq!(decoded.get_pixel(0, 0).0, [0, 0, 0, 0]);
    }

    /// RSM props reference TGAs, which carry real alpha and are never
    /// magenta-keyed by the runtime loader.
    #[test]
    fn tga_alpha_survives_and_magenta_is_not_keyed_out() {
        let magenta = [255, 0, 255, 255];
        let translucent = [10, 200, 10, 64];
        let mut image = RgbaImage::new(2, 1);
        image.put_pixel(0, 0, Rgba(magenta));
        image.put_pixel(1, 0, Rgba(translucent));
        let mut tga = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut tga), ImageFormat::Tga)
            .expect("encode synthetic tga");

        let ktx2_bytes = texture_bytes_to_ktx2("iz_rookie_06.tga", &tga).expect("convert");
        let reader = ktx2::Reader::new(&ktx2_bytes).expect("parse KTX2");
        let decoded = decode_base_level(&ktx2_bytes);

        assert_eq!(reader.header().format, Some(ktx2::Format::R8G8B8A8_SRGB));
        assert_eq!(decoded.get_pixel(0, 0).0, magenta);
        assert_eq!(decoded.get_pixel(1, 0).0, translucent);
    }

    #[test]
    fn an_unknown_texture_extension_fails_loudly() {
        let err = texture_bytes_to_ktx2("weird.dds", &[0u8; 8]).expect_err("must fail");

        let message = err.to_string();
        assert!(message.contains("weird.dds"), "unexpected error: {message}");
        assert!(message.contains("dds"), "unexpected error: {message}");
    }

    /// Some maps name a ground texture the archives do not contain. The map is
    /// otherwise complete, so it converts with an obviously wrong stand-in.
    #[test]
    fn a_missing_texture_becomes_a_placeholder() {
        let vfs = GrfVfs::open(&[] as &[&GrfEntry]).expect("empty vfs opens");
        let out = tempfile::tempdir().expect("tempdir");

        let textures = normalize_textures(&vfs, &["grass01.bmp".to_string()], out.path())
            .expect("a missing texture must not fail the map");

        assert_eq!(textures.len(), 1);
        assert_eq!(textures[0].relative_path, "tex/grass01_bmp.ktx2");

        let written = std::fs::read(out.path().join("tex/grass01_bmp.ktx2")).expect("placeholder");
        assert_eq!(
            written,
            crate::converters::model::fallback_texture_ktx2().expect("placeholder"),
            "a missing texture must use the shared placeholder"
        );
    }

    #[test]
    fn sanitize_name_flattens_directories_and_non_ascii() {
        assert_eq!(sanitize_name("grass01.bmp"), "grass01_bmp");
        assert_eq!(
            sanitize_name("sub\\dir\\Grass 01.bmp"),
            "sub_dir_grass_01_bmp"
        );
    }

    /// Two retail props reference `iz_rookie_06.bmp` and `iz_rookie_06.tga`:
    /// different images, so they must not share a pooled KTX2.
    #[test]
    fn the_same_stem_under_two_extensions_stays_distinct() {
        let names = vec![
            "izlude\\iz_rookie_06.bmp".to_string(),
            "izlude\\iz_rookie_06.tga".to_string(),
        ];

        let sanitized = assign_unique_sanitized_names(&names).expect("no collision");

        assert_ne!(sanitized[0], sanitized[1]);
    }

    #[test]
    fn distinct_subdirs_same_basename_produce_distinct_sanitized_names() {
        let names = vec!["floor\\dirt.bmp".to_string(), "wall\\dirt.bmp".to_string()];

        let sanitized = assign_unique_sanitized_names(&names).expect("no collision");

        assert_ne!(sanitized[0], sanitized[1]);
        assert_eq!(sanitized[0], "floor_dirt_bmp");
        assert_eq!(sanitized[1], "wall_dirt_bmp");
    }

    /// Half the GRF texture namespace is EUC-KR; stripping the non-ASCII
    /// characters alone collapses distinct names onto the same underscore run.
    #[test]
    fn distinct_korean_names_produce_distinct_sanitized_names() {
        let names = vec![
            "필드바닥\\prt_초원04.bmp".to_string(),
            "필드바닥\\prt_언덕04.bmp".to_string(),
        ];

        let sanitized = assign_unique_sanitized_names(&names).expect("no collision");

        assert_ne!(sanitized[0], sanitized[1]);
        assert!(sanitized[0].is_ascii(), "{}", sanitized[0]);
    }

    /// `verus\danger03.rsm` names one texture as both `ver_h_03.BMP` and
    /// `ver_h_03.bmp`. GRF lookup is case-insensitive, so those are the same
    /// file and must pool as one KTX2 rather than read as a collision.
    #[test]
    fn case_only_spelling_differences_are_one_texture() {
        let names = vec![
            "verus\\ver_h_03.BMP".to_string(),
            "verus\\ver_h_03.bmp".to_string(),
            "필드바닥\\PRT_초원04.bmp".to_string(),
            "필드바닥\\prt_초원04.bmp".to_string(),
        ];

        let sanitized = assign_unique_sanitized_names(&names).expect("same file, not a collision");

        assert_eq!(sanitized[0], sanitized[1]);
        assert_eq!(sanitized[2], sanitized[3]);
    }

    /// Water textures are JPEG and some maps use them as ground textures.
    /// Bevy's stock loader keys nothing, so neither does the converter.
    #[test]
    fn jpeg_and_png_decode_without_magenta_keying() {
        let mut image = image::RgbImage::new(1, 1);
        image.put_pixel(0, 0, image::Rgb([255, 0, 255]));

        for (name, format) in [
            ("워터\\water810.jpg", ImageFormat::Jpeg),
            ("x.png", ImageFormat::Png),
        ] {
            let mut bytes = Vec::new();
            image
                .write_to(&mut Cursor::new(&mut bytes), format)
                .expect("encode source");

            let decoded = decode_base_level(&texture_bytes_to_ktx2(name, &bytes).expect("convert"));

            assert_eq!(decoded.get_pixel(0, 0).0[3], 255, "{name} must stay opaque");
        }
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
