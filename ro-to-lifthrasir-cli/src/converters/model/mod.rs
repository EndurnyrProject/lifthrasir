//! Per-model entry point: reads one RSM out of the GRFs and writes the prop
//! glb mirroring its GRF path under `<models_dir>`, with every texture exported
//! once into the run-wide `<models_dir>/tex/` pool.

pub mod corpus;
#[cfg(test)]
pub mod fixtures;
pub mod mesh;
pub mod normalized;
pub mod rsm2;
pub mod validate;
pub mod writer;

use crate::converters::gltf_out::{hash_hex, to_forward_slashes};
use crate::converters::map::textures::{
    TextureOut, canonical_name, sanitize_name, texture_bytes_to_png,
};
use crate::grf_vfs::AssetRead;
use anyhow::{Context, anyhow, bail, ensure};
use ro_formats::{Rsm, Rsm2};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// RSM1 revisions the mesh builder and writer understand. RSM2 (`2.x`) is a
/// different container the native path does not read either.
const SUPPORTED_RSM1_MINORS: std::ops::RangeInclusive<u8> = 2..=5;
const SUPPORTED_RSM2_MINORS: std::ops::RangeInclusive<u8> = 2..=3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelFormat {
    Rsm1 { minor: u8 },
    Rsm2 { minor: u8 },
    Unsupported { major: u8, minor: u8 },
}

impl ModelFormat {
    pub fn version(self) -> (u8, u8) {
        match self {
            Self::Rsm1 { minor } => (1, minor),
            Self::Rsm2 { minor } => (2, minor),
            Self::Unsupported { major, minor } => (major, minor),
        }
    }
}

/// What `convert_model` did with one model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertOutcome {
    Converted,
    /// The target glb was already on disk and `force` was not set.
    Skipped,
    /// Not an RSM1 the converter supports; the caller keeps the native ref.
    UnsupportedVersion,
}

/// The glb path for a GRF model filename, relative to `models_dir`.
///
/// Lowercased and forward-slashed to match how the pak normalizes entries
/// (`grf-utils/src/pak.rs`), so the `ro://models/...` ref the map writer emits
/// resolves identically out of a pak and out of a loose data folder.
pub fn glb_relative_path(filename: &str) -> String {
    let path = to_forward_slashes(filename).to_lowercase();
    let stem = path
        .rsplit_once('.')
        .map_or(path.as_str(), |(base, _)| base);
    format!("{stem}.glb")
}

/// Convert `data/model/<filename>` into `<models_dir>/<path>.glb`, exporting
/// its textures through the run-wide `pool`.
pub fn convert_model(
    vfs: &impl AssetRead,
    filename: &str,
    models_dir: &Path,
    pool: &mut TexturePool,
    force: bool,
) -> anyhow::Result<ConvertOutcome> {
    let out_path = models_dir.join(glb_relative_path(filename));
    if !force && out_path.is_file() {
        return Ok(ConvertOutcome::Skipped);
    }
    if force {
        remove_existing_glb(&out_path)?;
    }

    let source_path = format!("data/model/{}", to_forward_slashes(filename));
    let bytes = vfs
        .read_asset(&source_path)
        .with_context(|| format!("model not found in GRFs: {source_path}"))?;
    convert_model_bytes(vfs, filename, &bytes, models_dir, pool, force)
}

/// Convert supplied physical model bytes while resolving texture dependencies
/// through the overlay `texture_source`.
pub fn convert_model_bytes(
    texture_source: &impl AssetRead,
    logical_path: &str,
    source_bytes: &[u8],
    models_dir: &Path,
    pool: &mut TexturePool,
    force: bool,
) -> anyhow::Result<ConvertOutcome> {
    let logical_path = to_forward_slashes(logical_path);
    let relative = glb_relative_path(&logical_path);
    let out_path = models_dir.join(&relative);
    if !force && out_path.is_file() {
        return Ok(ConvertOutcome::Skipped);
    }
    if force {
        remove_existing_glb(&out_path)?;
    }

    let format = classify_header(source_bytes)
        .with_context(|| format!("model {logical_path}, stage header"))?;
    let (major, minor) = format.version();
    let version = format!("{major}.{minor}");
    match format {
        ModelFormat::Unsupported { major: 1, .. } => {
            return Ok(ConvertOutcome::UnsupportedVersion);
        }
        ModelFormat::Unsupported { major: 2, .. } => bail!(
            "model {logical_path}, version {version}, stage dispatch: unsupported RSM2 version"
        ),
        ModelFormat::Unsupported { .. } => bail!(
            "model {logical_path}, version {version}, stage dispatch: unsupported model family"
        ),
        ModelFormat::Rsm1 { .. } | ModelFormat::Rsm2 { .. } => {}
    }

    let source_hash = hash_hex(source_bytes);
    let model = match format {
        ModelFormat::Rsm1 { .. } => {
            let source = Rsm::from_bytes(source_bytes).with_context(|| {
                format!("model {logical_path}, version {version}, stage parse RSM1")
            })?;
            mesh::build_model(&source, &source_hash).with_context(|| {
                format!("model {logical_path}, version {version}, stage normalize RSM1")
            })?
        }
        ModelFormat::Rsm2 { .. } => {
            let source = Rsm2::from_bytes(source_bytes).with_context(|| {
                format!("model {logical_path}, version {version}, stage parse RSM2")
            })?;
            rsm2::build_model(&source, &source_hash).with_context(|| {
                format!("model {logical_path}, version {version}, stage normalize RSM2")
            })?
        }
        ModelFormat::Unsupported { .. } => unreachable!("unsupported format passed dispatch"),
    };

    let textures = export_textures(
        texture_source,
        &model.textures,
        &relative,
        pool,
        matches!(format, ModelFormat::Rsm2 { .. }),
    )
    .with_context(|| format!("model {logical_path}, version {version}, stage export textures"))?;
    let parent = out_path
        .parent()
        .with_context(|| format!("model output path has no parent: {}", out_path.display()))?;
    std::fs::create_dir_all(parent).with_context(|| {
        format!("model {logical_path}, version {version}, stage create output directory")
    })?;

    let written = writer::write_model_glb(&out_path, &model, &textures)
        .with_context(|| format!("model {logical_path}, version {version}, stage write GLB"))
        .and_then(|()| {
            validate::validate(&out_path, &model, &textures).with_context(|| {
                format!("model {logical_path}, version {version}, stage validate GLB")
            })
        });
    if let Err(error) = written {
        return Err(remove_partial_glb(&out_path, error));
    }

    Ok(ConvertOutcome::Converted)
}

fn remove_existing_glb(out_path: &Path) -> anyhow::Result<()> {
    match std::fs::remove_file(out_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error)
            .with_context(|| format!("removing stale model output {}", out_path.display())),
    }
}

fn remove_partial_glb(out_path: &Path, error: anyhow::Error) -> anyhow::Error {
    match std::fs::remove_file(out_path) {
        Ok(()) => error,
        Err(cleanup) if cleanup.kind() == std::io::ErrorKind::NotFound => error,
        Err(cleanup) => anyhow!(
            "{error:#}; additionally failed to remove partial GLB {}: {cleanup}",
            out_path.display()
        ),
    }
}

/// Index-aligned with `rsm.textures`, as `write_model_glb` requires, with each
/// pool path rewritten relative to the glb's own directory.
fn export_textures(
    vfs: &impl AssetRead,
    texture_names: &[String],
    relative_glb_path: &str,
    pool: &mut TexturePool,
    replace_bik: bool,
) -> anyhow::Result<Vec<TextureOut>> {
    let up = "../".repeat(relative_glb_path.matches('/').count());

    texture_names
        .iter()
        .map(|name| {
            let texture = if replace_bik {
                pool.export_with_fallback(vfs, name)?
            } else {
                pool.export(vfs, name)?
            };
            Ok(TextureOut {
                relative_path: format!("{up}{}", texture.relative_path),
                ..texture
            })
        })
        .collect()
}

/// Reads the `GRSM` magic and exact version without parsing the body.
pub fn fallback_texture_png() -> anyhow::Result<Vec<u8>> {
    let pixels = vec![
        255, 0, 255, 255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 0, 255, 255,
    ];
    let image = image::RgbaImage::from_raw(2, 2, pixels).expect("fixed fallback image dimensions");
    let mut output = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut output, image::ImageFormat::Png)
        .context("encoding fallback texture")?;
    Ok(output.into_inner())
}

fn classify_header(bytes: &[u8]) -> anyhow::Result<ModelFormat> {
    let header = bytes
        .get(..6)
        .context("file is shorter than an RSM header")?;
    ensure!(&header[..4] == b"GRSM", "bad RSM magic: {:?}", &header[..4]);
    let (major, minor) = (header[4], header[5]);
    Ok(match major {
        1 if SUPPORTED_RSM1_MINORS.contains(&minor) => ModelFormat::Rsm1 { minor },
        2 if SUPPORTED_RSM2_MINORS.contains(&minor) => ModelFormat::Rsm2 { minor },
        _ => ModelFormat::Unsupported { major, minor },
    })
}

#[cfg(test)]
pub fn is_supported_version(bytes: &[u8]) -> anyhow::Result<bool> {
    Ok(matches!(
        classify_header(bytes)?,
        ModelFormat::Rsm1 { .. } | ModelFormat::Rsm2 { .. }
    ))
}

/// The `<models_dir>/tex/` PNG pool shared by every model in one run.
///
/// A texture is read and normalized at most once per run, and rewritten only
/// when its PNG is not already on disk from an earlier run. Two distinct source
/// names sanitizing to one filename abort the run, extending the per-map guard
/// in `map::textures::assign_unique_sanitized_names` to pool scope -- across
/// runs too, where the earlier source name is gone and the already-pooled bytes
/// are the only evidence left.
pub struct TexturePool {
    tex_dir: PathBuf,
    /// Sanitized filename stem -> the source name that claimed it.
    claimed: HashMap<String, String>,
    fallback_claims: HashSet<String>,
}

impl TexturePool {
    pub fn new(models_dir: &Path) -> Self {
        Self {
            tex_dir: models_dir.join("tex"),
            claimed: HashMap::new(),
            fallback_claims: HashSet::new(),
        }
    }

    /// The pooled PNG for `source_name`, relative to `models_dir`.
    pub fn export(
        &mut self,
        vfs: &impl AssetRead,
        source_name: &str,
    ) -> anyhow::Result<TextureOut> {
        self.export_with_policy(vfs, source_name, false)
    }

    fn export_with_fallback(
        &mut self,
        vfs: &impl AssetRead,
        source_name: &str,
    ) -> anyhow::Result<TextureOut> {
        self.export_with_policy(vfs, source_name, true)
    }

    fn export_with_policy(
        &mut self,
        vfs: &impl AssetRead,
        source_name: &str,
        replace_bik: bool,
    ) -> anyhow::Result<TextureOut> {
        let sanitized = sanitize_name(source_name);
        let texture = TextureOut {
            source_name: source_name.to_string(),
            relative_path: format!("tex/{sanitized}.png"),
        };
        match self.claimed.get(&sanitized) {
            Some(previous) if canonical_name(previous) == canonical_name(source_name) => {
                // A BIK stand-in written for an RSM2 consumer is replaced by the
                // real texture once a consumer that keeps BIK asks for it.
                if !replace_bik && self.fallback_claims.contains(&sanitized) {
                    self.write_png(vfs, source_name, &sanitized, false)?;
                }
                return Ok(texture);
            }
            Some(previous) => bail!(
                "texture name collision: '{source_name}' and '{previous}' both sanitize to '{sanitized}.png'"
            ),
            None => {}
        }
        let used_fallback = self.write_png(vfs, source_name, &sanitized, replace_bik)?;
        self.claimed
            .insert(sanitized.clone(), source_name.to_string());
        if used_fallback {
            self.fallback_claims.insert(sanitized);
        }
        Ok(texture)
    }

    fn write_png(
        &self,
        vfs: &impl AssetRead,
        source_name: &str,
        sanitized: &str,
        replace_bik: bool,
    ) -> anyhow::Result<bool> {
        let logical_path = format!("data/texture/{source_name}");
        let is_bik = source_name
            .rsplit_once('.')
            .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("bik"));
        let (png_bytes, used_fallback) = match vfs.read_asset(&logical_path) {
            Some(_) if replace_bik && is_bik => (fallback_texture_png()?, true),
            Some(source_bytes) => (
                texture_bytes_to_png(source_name, &source_bytes)
                    .with_context(|| format!("converting texture: {logical_path}"))?,
                false,
            ),
            // A texture the archives simply do not contain gets an obviously
            // wrong stand-in rather than failing the model, but the
            // substitution is reported so it cannot pass unnoticed.
            None => {
                println!("  missing texture, using a placeholder: {logical_path}");
                (fallback_texture_png()?, true)
            }
        };

        let dest = self.tex_dir.join(format!("{sanitized}.png"));
        if dest.is_file() {
            let pooled = std::fs::read(&dest)
                .with_context(|| format!("reading pooled texture: {}", dest.display()))?;
            ensure!(
                pooled == png_bytes,
                "texture name collision: '{source_name}' sanitizes to '{sanitized}.png', already pooled from a different source at {}",
                dest.display()
            );
            return Ok(used_fallback);
        }

        std::fs::create_dir_all(&self.tex_dir)
            .with_context(|| format!("creating {}", self.tex_dir.display()))?;
        std::fs::write(&dest, &png_bytes).with_context(|| format!("writing {}", dest.display()))?;
        Ok(used_fallback)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converters::model::fixtures::{FakeVfs, bmp_bytes, encode_rsm, encode_rsm2};
    use lifthrasir_data::lif;

    const TREE: &str = "prontera\\Tree01.rsm";
    const BUSH: &str = "prontera\\Bush01.rsm";

    const WHITE: [u8; 4] = [255, 255, 255, 255];
    const GREEN: [u8; 4] = [0, 255, 0, 255];

    fn vfs(models: &[(&str, Vec<u8>)], textures: &[&str]) -> FakeVfs {
        let files: Vec<(&str, Vec<u8>)> = models
            .iter()
            .map(|(path, bytes)| (*path, bytes.clone()))
            .chain(textures.iter().map(|name| (*name, bmp_bytes(WHITE))))
            .collect();
        FakeVfs::with(&files)
    }

    fn one_texture_model() -> Vec<(&'static str, Vec<u8>)> {
        vec![(
            "data/model/prontera/Tree01.rsm",
            encode_rsm((1, 4), &["bark.bmp"]),
        )]
    }

    #[test]
    fn glb_path_mirrors_the_grf_path_lowercased() {
        assert_eq!(
            glb_relative_path("prontera\\Tree01.rsm"),
            "prontera/tree01.glb"
        );
        assert_eq!(glb_relative_path("tree01.rsm"), "tree01.glb");
    }

    #[test]
    fn converts_a_model_into_a_path_mirrored_glb_with_pooled_textures() {
        let vfs = vfs(&one_texture_model(), &["data/texture/bark.bmp"]);
        let out = tempfile::tempdir().expect("tempdir");
        let mut pool = TexturePool::new(out.path());

        let outcome =
            convert_model(&vfs, TREE, out.path(), &mut pool, false).expect("convert model");

        assert_eq!(outcome, ConvertOutcome::Converted);
        assert!(out.path().join("prontera/tree01.glb").is_file());
        assert!(out.path().join("tex/bark_bmp.png").is_file());
    }

    /// The glb sits one directory below `models_dir`, so its image URIs have to
    /// climb back out to the shared pool.
    #[test]
    fn texture_uris_are_relative_to_the_glb_directory() {
        let vfs = vfs(&one_texture_model(), &["data/texture/bark.bmp"]);
        let out = tempfile::tempdir().expect("tempdir");
        let mut pool = TexturePool::new(out.path());

        convert_model(&vfs, TREE, out.path(), &mut pool, false).expect("convert model");

        let gltf::Gltf { document, .. } =
            gltf::Gltf::open(out.path().join("prontera/tree01.glb")).expect("reopen glb");
        let uris: Vec<&str> = document
            .images()
            .filter_map(|image| match image.source() {
                gltf::image::Source::Uri { uri, .. } => Some(uri),
                gltf::image::Source::View { .. } => None,
            })
            .collect();
        assert_eq!(uris, ["../tex/bark_bmp.png"]);
    }

    #[test]
    fn existing_glb_is_skipped_unless_forced() {
        let vfs = vfs(&one_texture_model(), &["data/texture/bark.bmp"]);
        let out = tempfile::tempdir().expect("tempdir");
        let mut pool = TexturePool::new(out.path());
        convert_model(&vfs, TREE, out.path(), &mut pool, false).expect("first convert");
        let reads = vfs.reads("data/model/prontera/Tree01.rsm");

        let skipped =
            convert_model(&vfs, TREE, out.path(), &mut pool, false).expect("second convert");
        assert_eq!(skipped, ConvertOutcome::Skipped);
        assert_eq!(vfs.reads("data/model/prontera/Tree01.rsm"), reads);

        let forced = convert_model(&vfs, TREE, out.path(), &mut pool, true).expect("forced");
        assert_eq!(forced, ConvertOutcome::Converted);
        assert_eq!(vfs.reads("data/model/prontera/Tree01.rsm"), reads + 1);
    }

    #[test]
    fn classifies_exact_rsm_families_and_versions() {
        assert_eq!(
            classify_header(b"GRSM\x01\x04").unwrap(),
            ModelFormat::Rsm1 { minor: 4 }
        );
        assert_eq!(
            classify_header(b"GRSM\x02\x03").unwrap(),
            ModelFormat::Rsm2 { minor: 3 }
        );
        assert_eq!(
            classify_header(b"GRSM\x02\x04").unwrap(),
            ModelFormat::Unsupported { major: 2, minor: 4 }
        );
    }

    #[test]
    fn converts_rsm2_2_2_and_2_3_by_header_with_pooled_textures() {
        for (minor, logical_path) in [(2, "prontera\\Tree01.rsm2"), (3, TREE)] {
            let source = encode_rsm2(minor, "bark.bmp");
            let vfs = vfs(&[], &["data/texture/bark.bmp"]);
            let out = tempfile::tempdir().expect("tempdir");
            let mut pool = TexturePool::new(out.path());

            let outcome =
                convert_model_bytes(&vfs, logical_path, &source, out.path(), &mut pool, false)
                    .expect("convert RSM2");

            assert_eq!(outcome, ConvertOutcome::Converted);
            let glb = out.path().join(glb_relative_path(logical_path));
            assert!(glb.is_file());
            assert!(out.path().join("tex/bark_bmp.png").is_file());
            let (document, _, _) = gltf::import(glb).expect("reimport");
            let root = document.into_json();
            let provenance: lif::LifModel = serde_json::from_value(
                root.extensions.as_ref().expect("extensions").others[lif::EXTENSION_MODEL].clone(),
            )
            .expect("LIF_model");
            assert_eq!(provenance.rsm_hash, hash_hex(&source));
        }
    }

    #[test]
    fn physical_bytes_win_over_overlay_model_and_skip_is_idempotent() {
        let physical = encode_rsm2(2, "bark.bmp");
        let overlay = encode_rsm2(3, "other.bmp");
        let vfs = vfs(
            &[("data/model/prontera/Tree01.rsm", overlay)],
            &["data/texture/bark.bmp"],
        );
        let out = tempfile::tempdir().expect("tempdir");
        let mut pool = TexturePool::new(out.path());

        assert_eq!(
            convert_model_bytes(&vfs, TREE, &physical, out.path(), &mut pool, false).unwrap(),
            ConvertOutcome::Converted
        );
        assert_eq!(vfs.reads("data/model/prontera/Tree01.rsm"), 0);
        assert_eq!(
            convert_model_bytes(&vfs, TREE, b"ignored", out.path(), &mut pool, false).unwrap(),
            ConvertOutcome::Skipped
        );
        assert_eq!(
            convert_model_bytes(&vfs, TREE, &physical, out.path(), &mut pool, true).unwrap(),
            ConvertOutcome::Converted
        );
    }

    #[test]
    fn only_unsupported_legacy_rsm1_falls_back() {
        let vfs = FakeVfs::default();
        let out = tempfile::tempdir().expect("tempdir");
        let mut pool = TexturePool::new(out.path());
        let legacy = b"GRSM\x01\x06";
        assert_eq!(
            convert_model_bytes(&vfs, TREE, legacy, out.path(), &mut pool, false).unwrap(),
            ConvertOutcome::UnsupportedVersion
        );

        for bytes in [b"GRSM\x02\x04".as_slice(), b"GRSM\x03\x00".as_slice()] {
            let error = convert_model_bytes(&vfs, TREE, bytes, out.path(), &mut pool, false)
                .expect_err("must fail");
            let message = format!("{error:#}");
            assert!(
                message.contains("prontera/Tree01.rsm"),
                "unexpected error: {message}"
            );
            assert!(
                message.contains("stage dispatch"),
                "unexpected error: {message}"
            );
        }
    }

    #[test]
    fn forced_failure_removes_stale_glb_before_parse_or_texture_export() {
        let source = encode_rsm2(2, "bark.bmp");
        let source_vfs = vfs(&[], &["data/texture/bark.bmp"]);
        let out = tempfile::tempdir().expect("tempdir");
        let mut pool = TexturePool::new(out.path());
        let glb = out.path().join(glb_relative_path(TREE));

        convert_model_bytes(&source_vfs, TREE, &source, out.path(), &mut pool, false).unwrap();
        assert!(glb.is_file());
        assert!(
            convert_model_bytes(
                &source_vfs,
                TREE,
                b"GRSM\x02\x03",
                out.path(),
                &mut pool,
                true,
            )
            .is_err()
        );
        assert!(!glb.exists());

        convert_model_bytes(&source_vfs, TREE, &source, out.path(), &mut pool, false).unwrap();
        assert!(glb.is_file());
        let unsupported_texture = encode_rsm2(2, "broken.webp");
        let broken_vfs = vfs(&[], &["data/texture/broken.webp"]);
        assert!(
            convert_model_bytes(
                &broken_vfs,
                TREE,
                &unsupported_texture,
                out.path(),
                &mut pool,
                true,
            )
            .is_err()
        );
        assert!(!glb.exists());
    }

    #[test]
    fn malformed_rsm2_is_fatal_with_version_and_stage() {
        let vfs = FakeVfs::default();
        let out = tempfile::tempdir().expect("tempdir");
        let mut pool = TexturePool::new(out.path());
        let error = convert_model_bytes(&vfs, TREE, b"GRSM\x02\x03", out.path(), &mut pool, false)
            .expect_err("must fail");
        let message = format!("{error:#}");
        assert!(message.contains("prontera/Tree01.rsm"));
        assert!(message.contains("version 2.3"));
        assert!(message.contains("stage parse RSM2"));
        assert!(!out.path().join("prontera/tree01.glb").exists());
    }

    #[test]
    fn corrupt_rsm1_fails_loudly() {
        let mut truncated = encode_rsm((1, 4), &["bark.bmp"]);
        truncated.truncate(64);
        let vfs = vfs(
            &[("data/model/prontera/Tree01.rsm", truncated)],
            &["data/texture/bark.bmp"],
        );
        let out = tempfile::tempdir().expect("tempdir");
        let mut pool = TexturePool::new(out.path());

        let err = convert_model(&vfs, TREE, out.path(), &mut pool, false).expect_err("must fail");

        assert!(
            err.to_string().contains("prontera/Tree01.rsm"),
            "unexpected error: {err}"
        );
    }

    /// A texture the archives do not contain is stood in for rather than
    /// failing the model - for RSM1 just as for RSM2.
    #[test]
    fn a_missing_rsm1_texture_becomes_a_placeholder() {
        let vfs = vfs(&one_texture_model(), &[]);
        let out = tempfile::tempdir().expect("tempdir");
        let mut pool = TexturePool::new(out.path());

        assert_eq!(
            convert_model(&vfs, TREE, out.path(), &mut pool, false).expect("must convert"),
            ConvertOutcome::Converted
        );

        let written = std::fs::read(out.path().join("tex/bark_bmp.png")).expect("placeholder");
        assert_eq!(written, fallback_texture_png().expect("placeholder"));
    }

    #[test]
    fn rsm2_missing_and_bik_textures_use_the_pinned_fallback() {
        for (source_name, include_source) in [("missing.bmp", false), ("screen.bik", true)] {
            let model = encode_rsm2(3, source_name);
            let textures = include_source
                .then_some(source_name)
                .into_iter()
                .collect::<Vec<_>>();
            let vfs = vfs(&[("data/model/fallback.rsm2", model)], &textures);
            let out = tempfile::tempdir().expect("tempdir");
            let mut pool = TexturePool::new(out.path());

            assert_eq!(
                convert_model(&vfs, "fallback.rsm2", out.path(), &mut pool, false,).unwrap(),
                ConvertOutcome::Converted
            );
            let png = std::fs::read(
                out.path()
                    .join(format!("tex/{}.png", sanitize_name(source_name))),
            )
            .unwrap();
            let image = image::load_from_memory_with_format(&png, image::ImageFormat::Png)
                .unwrap()
                .to_rgba8();
            assert_eq!(image.dimensions(), (2, 2));
            assert_eq!(image.get_pixel(0, 0).0, [255, 0, 255, 255]);
            assert_eq!(image.get_pixel(1, 0).0, [0, 0, 0, 255]);
        }
    }

    #[test]
    fn pool_writes_a_shared_texture_once_across_models() {
        let models = vec![
            (
                "data/model/prontera/Tree01.rsm",
                encode_rsm((1, 4), &["bark.bmp"]),
            ),
            (
                "data/model/prontera/Bush01.rsm",
                encode_rsm((1, 4), &["bark.bmp"]),
            ),
        ];
        let vfs = vfs(&models, &["data/texture/bark.bmp"]);
        let out = tempfile::tempdir().expect("tempdir");
        let mut pool = TexturePool::new(out.path());

        convert_model(&vfs, TREE, out.path(), &mut pool, false).expect("convert tree");
        convert_model(&vfs, BUSH, out.path(), &mut pool, false).expect("convert bush");

        assert_eq!(vfs.reads("data/texture/bark.bmp"), 1);
        assert!(out.path().join("prontera/bush01.glb").is_file());
    }

    /// A second CLI run over an already-populated `tex/` keeps the pooled PNG.
    #[test]
    fn a_png_pooled_by_an_earlier_run_from_the_same_source_is_reused() {
        let vfs = vfs(&one_texture_model(), &["data/texture/bark.bmp"]);
        let out = tempfile::tempdir().expect("tempdir");
        convert_model(
            &vfs,
            TREE,
            out.path(),
            &mut TexturePool::new(out.path()),
            false,
        )
        .expect("first run");
        let pooled = std::fs::read(out.path().join("tex/bark_bmp.png")).expect("pooled png");

        let outcome = convert_model(
            &vfs,
            TREE,
            out.path(),
            &mut TexturePool::new(out.path()),
            true,
        )
        .expect("second run");

        assert_eq!(outcome, ConvertOutcome::Converted);
        assert_eq!(
            std::fs::read(out.path().join("tex/bark_bmp.png")).expect("pooled png"),
            pooled
        );
    }

    /// The in-run memo is gone on the next run, so the pooled bytes are the only
    /// evidence that `tex/a_b.png` belongs to a different source. Reusing it
    /// would texture this model with another one's pixels.
    #[test]
    fn a_png_pooled_by_an_earlier_run_from_a_different_source_fails_loudly() {
        let vfs = FakeVfs::with(&[
            (
                "data/model/prontera/Tree01.rsm",
                encode_rsm((1, 4), &["a b.bmp"]),
            ),
            (
                "data/model/prontera/Bush01.rsm",
                encode_rsm((1, 4), &["a_b.bmp"]),
            ),
            ("data/texture/a b.bmp", bmp_bytes(WHITE)),
            ("data/texture/a_b.bmp", bmp_bytes(GREEN)),
        ]);
        let out = tempfile::tempdir().expect("tempdir");
        convert_model(
            &vfs,
            TREE,
            out.path(),
            &mut TexturePool::new(out.path()),
            false,
        )
        .expect("first run");

        let err = convert_model(
            &vfs,
            BUSH,
            out.path(),
            &mut TexturePool::new(out.path()),
            false,
        )
        .expect_err("must collide");

        let message = format!("{err:#}");
        assert!(
            message.contains("a_b_bmp.png"),
            "unexpected error: {message}"
        );
        assert!(message.contains("a_b.bmp"), "unexpected error: {message}");
    }

    /// `verus\danger03.rsm` names one texture twice, once `.BMP` and once
    /// `.bmp`. GRF lookup is case-insensitive, so both reads hit the same
    /// entry and the pool must claim it once instead of reporting a collision.
    #[test]
    fn case_only_spelling_differences_share_one_pooled_texture() {
        let models = vec![(
            "data/model/prontera/Tree01.rsm",
            encode_rsm((1, 4), &["ver_h_03.BMP", "ver_h_03.bmp"]),
        )];
        let vfs = vfs(&models, &["data/texture/ver_h_03.bmp"]);
        let out = tempfile::tempdir().expect("tempdir");
        let mut pool = TexturePool::new(out.path());

        convert_model(&vfs, TREE, out.path(), &mut pool, false)
            .expect("one file spelled two ways is not a collision");

        assert!(out.path().join("tex/ver_h_03_bmp.png").is_file());
    }

    #[test]
    fn sanitized_name_collision_across_models_fails_loudly() {
        let models = vec![
            (
                "data/model/prontera/Tree01.rsm",
                encode_rsm((1, 4), &["a b.bmp"]),
            ),
            (
                "data/model/prontera/Bush01.rsm",
                encode_rsm((1, 4), &["a_b.bmp"]),
            ),
        ];
        let vfs = vfs(&models, &["data/texture/a b.bmp", "data/texture/a_b.bmp"]);
        let out = tempfile::tempdir().expect("tempdir");
        let mut pool = TexturePool::new(out.path());
        convert_model(&vfs, TREE, out.path(), &mut pool, false).expect("convert tree");

        let err =
            convert_model(&vfs, BUSH, out.path(), &mut pool, false).expect_err("must collide");

        let message = format!("{err:#}");
        assert!(message.contains("a b.bmp"), "unexpected error: {message}");
        assert!(message.contains("a_b.bmp"), "unexpected error: {message}");
    }
}
