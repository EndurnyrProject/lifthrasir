//! GLB storage adapter. Document editing/history never performs filesystem I/O;
//! only the explicit `open` and `save` entry points cross this boundary.

use anyhow::{Context, Result, ensure};
use lifthrasir_data::lif::{self, LifEditor, LifMap};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use super::{MapDocument, MapSceneSummary, compact_deleted_props};

impl MapDocument {
    /// Opens a converted map GLB and validates the editable LIF payload.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let source_path = path.as_ref().to_path_buf();
        let bytes = std::fs::read(&source_path)
            .with_context(|| format!("reading map GLB {}", source_path.display()))?;
        let (json_chunk, bin_chunk) = split_glb(&bytes)?;
        let root: serde_json::Value =
            serde_json::from_slice(&json_chunk).context("parsing map GLB JSON chunk")?;
        Self::from_json_root(source_path, &root, json_chunk, bin_chunk)
    }

    /// Atomically publishes collision, transform and prop-structure edits.
    /// The in-memory node identities/history remain stable across saves.
    pub fn save(&mut self) -> Result<()> {
        self.commit_pending_prop_transform();
        let collision = self.editor.collision;
        let bytes =
            lif::LifCollision::encode(&self.collision_cells, collision.width, collision.height)
                .map_err(anyhow::Error::msg)?;
        let compacted = compact_deleted_props(&self.root, &self.deleted_props)?;
        self.json_chunk = serde_json::to_vec(&compacted).context("serializing edited map JSON")?;
        let (start, end) = collision_range(&self.json_chunk, collision)?;
        ensure!(
            end - start == bytes.len(),
            "LIF_editor collision storage changed size; a GLB rebuild is required"
        );
        self.bin_chunk[start..end].copy_from_slice(&bytes);
        write_atomic(
            &self.source_path,
            &glb_container(&self.json_chunk, &self.bin_chunk),
        )?;
        self.dirty = false;
        Ok(())
    }

    fn from_json_root(
        source_path: PathBuf,
        root: &serde_json::Value,
        json_chunk: Vec<u8>,
        bin_chunk: Vec<u8>,
    ) -> Result<Self> {
        let map: LifMap = root_extension(root, lif::EXTENSION_MAP)?;
        ensure!(
            map.format_version == lif::FORMAT_VERSION,
            "{} format version {} is not supported (editor expects {})",
            lif::EXTENSION_MAP,
            map.format_version,
            lif::FORMAT_VERSION,
        );
        let editor: LifEditor = root_extension(root, lif::EXTENSION_EDITOR)?;
        let (start, end) = collision_range(&json_chunk, editor.collision)?;
        ensure!(
            end <= bin_chunk.len(),
            "LIF_editor collision buffer lies outside the GLB BIN chunk"
        );
        let collision_cells = editor
            .collision
            .decode(&bin_chunk[start..end])
            .map_err(anyhow::Error::msg)?;
        Ok(Self {
            source_path,
            editor,
            water: optional_root_extension(root, lif::EXTENSION_WATER)?,
            map,
            scene: summarize_scene(root),
            collision_cells,
            undo: Vec::new(),
            redo: Vec::new(),
            pending_prop_transform: None,
            deleted_props: BTreeSet::new(),
            dirty: false,
            root: root.clone(),
            json_chunk,
            bin_chunk,
        })
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("map GLB has no parent directory")?;
    let temporary = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("creating temporary map beside {}", path.display()))?;
    std::fs::write(temporary.path(), bytes)
        .with_context(|| format!("writing temporary map {}", temporary.path().display()))?;
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("publishing edited map {}", path.display()))?;
    Ok(())
}

const GLB_MAGIC: &[u8; 4] = b"glTF";
const JSON_CHUNK: u32 = 0x4E4F_534A;
const BIN_CHUNK: u32 = 0x004E_4942;

fn split_glb(bytes: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
    ensure!(bytes.len() >= 20, "GLB is shorter than its header");
    ensure!(&bytes[..4] == GLB_MAGIC, "file is not a GLB");
    ensure!(
        u32::from_le_bytes(bytes[4..8].try_into().expect("four bytes")) == 2,
        "unsupported GLB version"
    );
    ensure!(
        u32::from_le_bytes(bytes[8..12].try_into().expect("four bytes")) as usize == bytes.len(),
        "GLB header length does not match file length"
    );
    let json_len = u32::from_le_bytes(bytes[12..16].try_into().expect("four bytes")) as usize;
    ensure!(
        u32::from_le_bytes(bytes[16..20].try_into().expect("four bytes")) == JSON_CHUNK,
        "GLB first chunk is not JSON"
    );
    let json_end = 20usize
        .checked_add(json_len)
        .context("GLB JSON length overflows")?;
    ensure!(bytes.len() >= json_end + 8, "GLB has no BIN chunk");
    let bin_len = u32::from_le_bytes(
        bytes[json_end..json_end + 4]
            .try_into()
            .expect("four bytes"),
    ) as usize;
    ensure!(
        u32::from_le_bytes(
            bytes[json_end + 4..json_end + 8]
                .try_into()
                .expect("four bytes")
        ) == BIN_CHUNK,
        "GLB second chunk is not BIN"
    );
    let bin_start = json_end + 8;
    let bin_end = bin_start
        .checked_add(bin_len)
        .context("GLB BIN length overflows")?;
    ensure!(bin_end == bytes.len(), "GLB has trailing or truncated data");
    Ok((
        bytes[20..json_end].to_vec(),
        bytes[bin_start..bin_end].to_vec(),
    ))
}

fn collision_range(json_chunk: &[u8], collision: lif::LifCollision) -> Result<(usize, usize)> {
    let root: serde_json::Value =
        serde_json::from_slice(json_chunk).context("parsing GLB JSON chunk")?;
    let view = root
        .get("bufferViews")
        .and_then(serde_json::Value::as_array)
        .and_then(|views| views.get(collision.buffer_view))
        .with_context(|| {
            format!(
                "LIF_editor points at missing bufferView {}",
                collision.buffer_view
            )
        })?;
    let start = view
        .get("byteOffset")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0) as usize;
    let length = view
        .get("byteLength")
        .and_then(serde_json::Value::as_u64)
        .context("LIF_editor collision bufferView has no byteLength")? as usize;
    let end = start
        .checked_add(length)
        .context("LIF_editor collision buffer range overflows")?;
    Ok((start, end))
}

pub(super) fn glb_container(json: &[u8], bin: &[u8]) -> Vec<u8> {
    let mut json = json.to_vec();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let mut bin = bin.to_vec();
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    let total = 12 + 8 + json.len() + 8 + bin.len();
    let mut output = Vec::with_capacity(total);
    output.extend_from_slice(GLB_MAGIC);
    output.extend_from_slice(&2u32.to_le_bytes());
    output.extend_from_slice(&(total as u32).to_le_bytes());
    output.extend_from_slice(&(json.len() as u32).to_le_bytes());
    output.extend_from_slice(&JSON_CHUNK.to_le_bytes());
    output.extend_from_slice(&json);
    output.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    output.extend_from_slice(&BIN_CHUNK.to_le_bytes());
    output.extend_from_slice(&bin);
    output
}

fn root_extension<T: serde::de::DeserializeOwned>(
    root: &serde_json::Value,
    key: &str,
) -> Result<T> {
    let value = root
        .pointer(&format!("/extensions/{key}"))
        .with_context(|| format!("map is missing required root extension {key}"))?;
    serde_json::from_value(value.clone()).with_context(|| format!("decoding root extension {key}"))
}

fn optional_root_extension<T: serde::de::DeserializeOwned>(
    root: &serde_json::Value,
    key: &str,
) -> Result<Option<T>> {
    let Some(value) = root.pointer(&format!("/extensions/{key}")) else {
        return Ok(None);
    };
    serde_json::from_value(value.clone())
        .map(Some)
        .with_context(|| format!("decoding root extension {key}"))
}

fn summarize_scene(root: &serde_json::Value) -> MapSceneSummary {
    let terrain_primitives = root
        .get("meshes")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|mesh| mesh.get("primitives").and_then(serde_json::Value::as_array))
        .map(Vec::len)
        .sum();
    root.get("nodes")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .fold(
            MapSceneSummary {
                terrain_primitives,
                ..Default::default()
            },
            |mut summary, node| {
                let extras = node.get("extras").unwrap_or(&serde_json::Value::Null);
                summary.props += extras.get(lif::EXTRAS_PROP).is_some() as usize;
                summary.effects += extras.get(lif::EXTRAS_EFFECT).is_some() as usize;
                summary.audio_emitters += extras.get(lif::EXTRAS_AUDIO).is_some() as usize;
                summary.lights += node
                    .pointer("/extensions/KHR_lights_punctual/light")
                    .is_some() as usize;
                summary
            },
        )
}
