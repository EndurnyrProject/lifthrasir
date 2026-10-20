use anyhow::{Context, Result, ensure};
use lifthrasir_data::lif::{self, LifCollisionCell, LifEditor, LifMap, LifWater};
use std::path::{Path, PathBuf};

/// A converted Lifthrasir map opened independently of its Ragnarok sources.
///
/// The source path is retained only to support an eventual atomic GLB save;
/// all editable map data is read from the GLB itself.
#[derive(Debug, Clone)]
pub struct MapDocument {
    pub source_path: PathBuf,
    pub map: LifMap,
    pub editor: LifEditor,
    pub water: Option<LifWater>,
    pub scene: MapSceneSummary,
    collision_cells: Vec<LifCollisionCell>,
    undo: Vec<CollisionEdit>,
    redo: Vec<CollisionEdit>,
    dirty: bool,
    json_chunk: Vec<u8>,
    bin_chunk: Vec<u8>,
}

/// Counts useful to an editor before it materializes a scene.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MapSceneSummary {
    pub terrain_primitives: usize,
    pub lights: usize,
    pub props: usize,
    pub effects: usize,
    pub audio_emitters: usize,
}

#[derive(Debug, Clone, Copy)]
struct CollisionEdit {
    index: usize,
    before: LifCollisionCell,
    after: LifCollisionCell,
}

impl MapDocument {
    /// Opens a Lifthrasir map GLB and verifies its editor-relevant contract.
    ///
    /// This intentionally does not read the embedded legacy GAT buffer. The
    /// editor's next format migration will replace that buffer with editable
    /// LIF collision data; keeping document discovery independent of it makes
    /// that migration local to the collision subsystem.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let source_path = path.as_ref().to_path_buf();
        let bytes = std::fs::read(&source_path)
            .with_context(|| format!("reading map GLB {}", source_path.display()))?;
        let (json_chunk, bin_chunk) = split_glb(&bytes)?;
        let root: serde_json::Value =
            serde_json::from_slice(&json_chunk).context("parsing map GLB JSON chunk")?;
        Self::from_json_root(source_path, &root, json_chunk, bin_chunk)
    }

    /// Returns the cell at zero-based collision-grid coordinates.
    pub fn collision_cell(&self, x: u32, y: u32) -> Option<LifCollisionCell> {
        let width = self.editor.collision.width as usize;
        self.collision_cells
            .get(y as usize * width + x as usize)
            .copied()
    }

    /// Whether the document has collision edits that are not on disk yet.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Replaces one collision cell. Call [`Self::save`] to publish the change.
    pub fn set_collision_cell(&mut self, x: u32, y: u32, cell: LifCollisionCell) -> Result<()> {
        let collision = self.editor.collision;
        ensure!(
            x < collision.width && y < collision.height,
            "collision cell ({x}, {y}) is outside {}x{}",
            collision.width,
            collision.height
        );
        let index = y as usize * collision.width as usize + x as usize;
        let before = self.collision_cells[index];
        if before == cell {
            return Ok(());
        }
        self.collision_cells[index] = cell;
        self.undo.push(CollisionEdit {
            index,
            before,
            after: cell,
        });
        self.redo.clear();
        self.dirty = true;
        Ok(())
    }

    /// Reverts the latest collision-cell edit. Returns `true` when an edit was
    /// available to undo.
    pub fn undo(&mut self) -> bool {
        let Some(edit) = self.undo.pop() else {
            return false;
        };
        self.collision_cells[edit.index] = edit.before;
        self.redo.push(edit);
        self.dirty = true;
        true
    }

    /// Reapplies the latest reverted collision-cell edit.
    pub fn redo(&mut self) -> bool {
        let Some(edit) = self.redo.pop() else {
            return false;
        };
        self.collision_cells[edit.index] = edit.after;
        self.undo.push(edit);
        self.dirty = true;
        true
    }

    /// Atomically writes collision edits back to the source GLB.
    ///
    /// This first save path only changes the fixed-size `LIF_editor` buffer;
    /// terrain/object editing will use a JSON-and-buffer rebuild path later.
    pub fn save(&mut self) -> Result<()> {
        let collision = self.editor.collision;
        let bytes =
            lif::LifCollision::encode(&self.collision_cells, collision.width, collision.height)
                .map_err(anyhow::Error::msg)?;
        let (start, end) = collision_range(&self.json_chunk, collision)?;
        ensure!(
            end - start == bytes.len(),
            "LIF_editor collision storage changed size; a GLB rebuild is required"
        );
        self.bin_chunk[start..end].copy_from_slice(&bytes);

        let parent = self
            .source_path
            .parent()
            .context("map GLB has no parent directory")?;
        let temporary = tempfile::NamedTempFile::new_in(parent).with_context(|| {
            format!(
                "creating temporary map beside {}",
                self.source_path.display()
            )
        })?;
        std::fs::write(
            temporary.path(),
            glb_container(&self.json_chunk, &self.bin_chunk),
        )
        .with_context(|| format!("writing temporary map {}", temporary.path().display()))?;
        temporary
            .persist(&self.source_path)
            .map_err(|error| error.error)
            .with_context(|| format!("publishing edited map {}", self.source_path.display()))?;
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
            dirty: false,
            json_chunk,
            bin_chunk,
        })
    }
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

fn glb_container(json: &[u8], bin: &[u8]) -> Vec<u8> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn root(format_version: u32) -> serde_json::Value {
        json!({
            "extensions": {
                "LIF_map": {
                    "format_version": format_version,
                    "rsw_hash": "r",
                    "gnd_hash": "g",
                    "gat_hash": "a",
                    "ambient_color": [0.0, 0.0, 0.0]
                },
                "LIF_editor": {
                    "collision": { "width": 2, "height": 3, "buffer_view": 0 }
                }
            },
            "meshes": [{ "primitives": [{}, {}] }],
            "nodes": [
                { "extensions": { "KHR_lights_punctual": { "light": 0 } } },
                { "extras": { "lif_prop": {} } },
                { "extras": { "lif_effect": {} } },
                { "extras": { "lif_audio": {} } }
            ],
            "bufferViews": [{ "byteOffset": 0, "byteLength": 102 }]
        })
    }

    fn write_document(root: serde_json::Value) -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("temp directory");
        let cells = vec![
            LifCollisionCell {
                height: [0.0; 4],
                flags: lif::LifCollisionFlags::WALKABLE,
            };
            6
        ];
        let bin = lif::LifCollision::encode(&cells, 2, 3).expect("collision bytes");
        let json = serde_json::to_vec(&root).expect("json");
        std::fs::write(directory.path().join("map.glb"), glb_container(&json, &bin))
            .expect("map GLB");
        directory
    }

    #[test]
    fn discovers_editor_entities_without_native_map_files() {
        let directory = write_document(root(lif::FORMAT_VERSION));
        let document =
            MapDocument::open(directory.path().join("map.glb")).expect("LIF map document");

        assert_eq!(document.editor.collision.width, 2);
        assert_eq!(document.scene.terrain_primitives, 2);
        assert_eq!(document.scene.lights, 1);
        assert_eq!(document.scene.props, 1);
        assert_eq!(document.scene.effects, 1);
        assert_eq!(document.scene.audio_emitters, 1);
    }

    #[test]
    fn rejects_a_map_from_another_lif_format_version() {
        let directory = write_document(root(0));
        let error = MapDocument::open(directory.path().join("map.glb"))
            .expect_err("stale format must fail");

        assert!(error.to_string().contains("format version 0"));
    }

    #[test]
    fn collision_edits_undo_and_redo_without_writing_a_native_map() {
        let directory = write_document(root(lif::FORMAT_VERSION));
        let mut document = MapDocument::open(directory.path().join("map.glb")).expect("document");
        let original = document.collision_cell(1, 2).expect("cell");
        let edited = LifCollisionCell {
            flags: lif::LifCollisionFlags::empty(),
            ..original
        };

        document.set_collision_cell(1, 2, edited).expect("edit");
        assert!(document.is_dirty());
        assert_eq!(document.collision_cell(1, 2), Some(edited));
        assert!(document.undo());
        assert_eq!(document.collision_cell(1, 2), Some(original));
        assert!(document.redo());
        assert_eq!(document.collision_cell(1, 2), Some(edited));
    }
}
