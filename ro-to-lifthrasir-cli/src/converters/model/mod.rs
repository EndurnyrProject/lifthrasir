//! Per-model entry point: reads one RSM out of the GRFs and writes the prop
//! glb mirroring its GRF path under `<models_dir>`, with every texture exported
//! once into the run-wide `<models_dir>/tex/` pool.

pub mod corpus;
#[cfg(test)]
pub mod fixtures;
pub mod mesh;
pub mod validate;
pub mod writer;

use crate::converters::gltf_out::{hash_hex, to_forward_slashes};
use crate::converters::map::textures::{TextureOut, sanitize_name, texture_bytes_to_png};
use crate::grf_vfs::AssetRead;
use anyhow::{Context, bail, ensure};
use ro_formats::Rsm;
use std::collections::HashMap;
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
    let relative = glb_relative_path(filename);
    let out_path = models_dir.join(&relative);
    if !force && out_path.is_file() {
        return Ok(ConvertOutcome::Skipped);
    }

    let source_path = format!("data/model/{}", to_forward_slashes(filename));
    let bytes = vfs
        .read_asset(&source_path)
        .with_context(|| format!("model not found in GRFs: {source_path}"))?;

    let supported = is_supported_version(&bytes)
        .with_context(|| format!("reading RSM header of {source_path}"))?;
    if !supported {
        return Ok(ConvertOutcome::UnsupportedVersion);
    }

    let rsm =
        Rsm::from_bytes(&bytes).with_context(|| format!("parsing RSM model: {source_path}"))?;
    let build = mesh::build_model(&rsm)
        .with_context(|| format!("building meshes of model: {source_path}"))?;

    let textures = export_textures(vfs, &rsm, &relative, pool)
        .with_context(|| format!("exporting textures of model: {source_path}"))?;

    let parent = out_path
        .parent()
        .with_context(|| format!("model output path has no parent: {}", out_path.display()))?;
    std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;

    let written = writer::write_model_glb(&out_path, &rsm, &hash_hex(&bytes), &build, &textures)
        .and_then(|()| {
            validate::validate(&out_path, &build, rsm.anim_len)
                .with_context(|| format!("validating {}", out_path.display()))
        });
    if let Err(error) = written {
        // A half-written or invalid glb left behind would be taken for a good
        // one -- and skipped -- by the next run.
        std::fs::remove_file(&out_path).ok();
        return Err(error);
    }

    Ok(ConvertOutcome::Converted)
}

/// Index-aligned with `rsm.textures`, as `write_model_glb` requires, with each
/// pool path rewritten relative to the glb's own directory.
fn export_textures(
    vfs: &impl AssetRead,
    rsm: &Rsm,
    relative_glb_path: &str,
    pool: &mut TexturePool,
) -> anyhow::Result<Vec<TextureOut>> {
    let up = "../".repeat(relative_glb_path.matches('/').count());

    rsm.textures
        .iter()
        .map(|name| {
            let texture = pool.export(vfs, name)?;
            Ok(TextureOut {
                relative_path: format!("{up}{}", texture.relative_path),
                ..texture
            })
        })
        .collect()
}

/// Reads the `GRSM` magic and exact version without parsing the body.
pub fn classify_header(bytes: &[u8]) -> anyhow::Result<ModelFormat> {
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

pub fn is_supported_version(bytes: &[u8]) -> anyhow::Result<bool> {
    Ok(matches!(classify_header(bytes)?, ModelFormat::Rsm1 { .. }))
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
}

impl TexturePool {
    pub fn new(models_dir: &Path) -> Self {
        Self {
            tex_dir: models_dir.join("tex"),
            claimed: HashMap::new(),
        }
    }

    /// The pooled PNG for `source_name`, relative to `models_dir`.
    pub fn export(
        &mut self,
        vfs: &impl AssetRead,
        source_name: &str,
    ) -> anyhow::Result<TextureOut> {
        let sanitized = sanitize_name(source_name);
        let texture = TextureOut {
            source_name: source_name.to_string(),
            relative_path: format!("tex/{sanitized}.png"),
        };

        match self.claimed.get(&sanitized) {
            Some(previous) if previous == source_name => return Ok(texture),
            Some(previous) => bail!(
                "texture name collision: '{source_name}' and '{previous}' both sanitize to '{sanitized}.png'"
            ),
            None => {}
        }

        self.write_png(vfs, source_name, &sanitized)?;
        self.claimed.insert(sanitized, source_name.to_string());

        Ok(texture)
    }

    fn write_png(
        &self,
        vfs: &impl AssetRead,
        source_name: &str,
        sanitized: &str,
    ) -> anyhow::Result<()> {
        let logical_path = format!("data/texture/{source_name}");
        let source_bytes = vfs
            .read_asset(&logical_path)
            .with_context(|| format!("texture not found in GRFs: {logical_path}"))?;
        let png_bytes = texture_bytes_to_png(source_name, &source_bytes)
            .with_context(|| format!("converting texture: {logical_path}"))?;

        let dest = self.tex_dir.join(format!("{sanitized}.png"));
        if dest.is_file() {
            let pooled = std::fs::read(&dest)
                .with_context(|| format!("reading pooled texture: {}", dest.display()))?;
            ensure!(
                pooled == png_bytes,
                "texture name collision: '{source_name}' sanitizes to '{sanitized}.png', already pooled from a different source at {}",
                dest.display()
            );
            return Ok(());
        }

        std::fs::create_dir_all(&self.tex_dir)
            .with_context(|| format!("creating {}", self.tex_dir.display()))?;
        std::fs::write(&dest, &png_bytes).with_context(|| format!("writing {}", dest.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converters::model::fixtures::{FakeVfs, bmp_bytes, encode_rsm};

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
    fn rsm2_is_reported_as_unsupported_not_an_error() {
        let models = vec![(
            "data/model/prontera/Tree01.rsm",
            encode_rsm((2, 2), &["bark.bmp"]),
        )];
        let vfs = vfs(&models, &["data/texture/bark.bmp"]);
        let out = tempfile::tempdir().expect("tempdir");
        let mut pool = TexturePool::new(out.path());

        let outcome = convert_model(&vfs, TREE, out.path(), &mut pool, false).expect("no error");

        assert_eq!(outcome, ConvertOutcome::UnsupportedVersion);
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

    #[test]
    fn missing_texture_fails_loudly() {
        let vfs = vfs(&one_texture_model(), &[]);
        let out = tempfile::tempdir().expect("tempdir");
        let mut pool = TexturePool::new(out.path());

        let err = convert_model(&vfs, TREE, out.path(), &mut pool, false).expect_err("must fail");

        assert!(
            format!("{err:#}").contains("data/texture/bark.bmp"),
            "unexpected error: {err:#}"
        );
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
