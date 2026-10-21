use crate::converters::ktx2_out::encode_ktx2;
use anyhow::{Context, bail};
use image::{ImageFormat, RgbaImage};

/// Runtime magenta-key thresholds, mirrored from
/// `crates/lifthrasir-assets/src/loaders/pixels.rs::apply_magenta_transparency`.
const MAGENTA_THRESHOLD: u8 = 240;
const GREEN_THRESHOLD: u8 = 15;

/// The absolute `ro://` prefix every pooled ground texture URI starts with.
/// The pool itself is rooted at `<ground_dir>` (`assets/data/ground` by
/// default), so the files live in `ground/tex/` in the runtime namespace.
pub const GROUND_TEXTURE_URI_PREFIX: &str = "ro://ground/tex/";

/// A GND texture name mapped to the KTX2 written for it. `relative_path` is
/// the absolute `ro://` URI the map glb references.
#[derive(Debug, Clone)]
pub struct TextureOut {
    pub source_name: String,
    pub relative_path: String,
}

/// Exports every GND-referenced texture through the run-wide ground pool and
/// rewrites the result to the absolute `ro://ground/tex/` URI the map glb
/// stores. The pool keys out magenta to real alpha, dedupes across maps and
/// rejects colliding names.
pub fn pool_textures(
    vfs: &impl crate::grf_vfs::AssetRead,
    texture_names: &[String],
    pool: &crate::converters::model::TexturePool,
) -> anyhow::Result<Vec<TextureOut>> {
    texture_names
        .iter()
        .map(|name| {
            let pooled = pool.export(vfs, name)?;
            Ok(TextureOut {
                source_name: pooled.source_name,
                relative_path: format!("{GROUND_TEXTURE_URI_PREFIX}{}.ktx2", sanitize_name(name)),
            })
        })
        .collect()
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
/// appended to keep them apart. `TexturePool::export` stays the loud
/// backstop for names that still collide.
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
        let vfs = crate::converters::model::fixtures::FakeVfs::with(&[]);
        let out = tempfile::tempdir().expect("tempdir");
        let pool = crate::converters::model::TexturePool::new(out.path());

        let textures = pool_textures(&vfs, &["grass01.bmp".to_string()], &pool)
            .expect("a missing texture must not fail the map");

        assert_eq!(textures.len(), 1);
        assert_eq!(
            textures[0].relative_path,
            "ro://ground/tex/grass01_bmp.ktx2"
        );

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
        let names = [
            "izlude\\iz_rookie_06.bmp".to_string(),
            "izlude\\iz_rookie_06.tga".to_string(),
        ];

        assert_ne!(sanitize_name(&names[0]), sanitize_name(&names[1]));
    }

    #[test]
    fn distinct_subdirs_same_basename_produce_distinct_sanitized_names() {
        let names = ["floor\\dirt.bmp".to_string(), "wall\\dirt.bmp".to_string()];

        assert_ne!(sanitize_name(&names[0]), sanitize_name(&names[1]));
        assert_eq!(sanitize_name(&names[0]), "floor_dirt_bmp");
        assert_eq!(sanitize_name(&names[1]), "wall_dirt_bmp");
    }

    /// Half the GRF texture namespace is EUC-KR; stripping the non-ASCII
    /// characters alone collapses distinct names onto the same underscore run.
    #[test]
    fn distinct_korean_names_produce_distinct_sanitized_names() {
        let names = [
            "필드바닥\\prt_초원04.bmp".to_string(),
            "필드바닥\\prt_언덕04.bmp".to_string(),
        ];

        let left = sanitize_name(&names[0]);
        let right = sanitize_name(&names[1]);

        assert_ne!(left, right);
        assert!(left.is_ascii(), "{left}");
    }

    /// `verus\danger03.rsm` names one texture as both `ver_h_03.BMP` and
    /// `ver_h_03.bmp`. GRF lookup is case-insensitive, so those are the same
    /// file and must pool as one KTX2 rather than read as a collision.
    #[test]
    fn case_only_spelling_differences_are_one_texture() {
        let names = [
            "verus\\ver_h_03.BMP".to_string(),
            "verus\\ver_h_03.bmp".to_string(),
            "필드바닥\\PRT_초원04.bmp".to_string(),
            "필드바닥\\prt_초원04.bmp".to_string(),
        ];

        assert_eq!(canonical_name(&names[0]), canonical_name(&names[1]));
        assert_eq!(canonical_name(&names[2]), canonical_name(&names[3]));
        assert_eq!(sanitize_name(&names[0]), sanitize_name(&names[1]));
        assert_eq!(sanitize_name(&names[2]), sanitize_name(&names[3]));
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
    fn pool_textures_rejects_a_collision_across_distinct_sources() {
        let vfs = crate::converters::model::fixtures::FakeVfs::with(&[]);
        let out = tempfile::tempdir().expect("tempdir");
        let pool = crate::converters::model::TexturePool::new(out.path());
        let names = vec!["a b.bmp".to_string(), "a_b.bmp".to_string()];

        let err = pool_textures(&vfs, &names, &pool).expect_err("must collide");

        let message = err.to_string();
        assert!(message.contains("a b.bmp"), "unexpected error: {message}");
        assert!(message.contains("a_b.bmp"), "unexpected error: {message}");
    }

    /// A texture shared by two maps in one run is encoded and written once.
    #[test]
    fn a_pooled_texture_is_written_once_per_run() {
        let vfs = crate::converters::model::fixtures::FakeVfs::with(&[(
            "data/texture/grass01.bmp",
            crate::converters::model::fixtures::bmp_bytes([255, 255, 255, 255]),
        )]);
        let out = tempfile::tempdir().expect("tempdir");
        let pool = crate::converters::model::TexturePool::new(out.path());

        let names = vec!["grass01.bmp".to_string(); 2];
        let textures = pool_textures(&vfs, &names, &pool).expect("two maps share one texture");

        assert_eq!(vfs.reads("data/texture/grass01.bmp"), 1);
        assert_eq!(
            textures[0].relative_path,
            "ro://ground/tex/grass01_bmp.ktx2"
        );
        assert_eq!(textures[0].relative_path, textures[1].relative_path);
        assert!(out.path().join("tex/grass01_bmp.ktx2").is_file());
    }
}
