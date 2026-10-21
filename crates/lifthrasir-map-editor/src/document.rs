use anyhow::{Context, Result, ensure};
use lifthrasir_data::lif::{self, LifCollisionCell, LifEditor, LifMap, LifWater};
use std::collections::BTreeSet;
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
    undo: Vec<DocumentEdit>,
    redo: Vec<DocumentEdit>,
    pending_prop_transform: Option<PropTransformEdit>,
    deleted_props: BTreeSet<usize>,
    dirty: bool,
    root: serde_json::Value,
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
enum DocumentEdit {
    Collision {
        index: usize,
        before: LifCollisionCell,
        after: LifCollisionCell,
    },
    PropTransform {
        node_index: usize,
        before: NodeTransform,
        after: NodeTransform,
    },
    PropDeleted {
        node_index: usize,
        before: bool,
        after: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct NodeTransform {
    translation: [f32; 3],
    rotation: [f32; 4],
    scale: [f32; 3],
}

#[derive(Debug, Clone, Copy)]
struct PropTransformEdit {
    node_index: usize,
    before: NodeTransform,
    after: NodeTransform,
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
        self.undo.push(DocumentEdit::Collision {
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
        self.commit_pending_prop_transform();
        let Some(edit) = self.undo.pop() else {
            return false;
        };
        self.apply_edit(edit, false);
        self.redo.push(edit);
        self.dirty = true;
        true
    }

    /// Reapplies the latest reverted collision-cell edit.
    pub fn redo(&mut self) -> bool {
        self.commit_pending_prop_transform();
        let Some(edit) = self.redo.pop() else {
            return false;
        };
        self.apply_edit(edit, true);
        self.undo.push(edit);
        self.dirty = true;
        true
    }

    /// Replaces a map prop node's glTF local transform. `node_index` is the
    /// original GLB node identity exposed by the runtime, not an ECS entity.
    pub fn set_prop_transform(
        &mut self,
        node_index: usize,
        translation: [f32; 3],
        rotation: [f32; 4],
        scale: [f32; 3],
    ) -> Result<()> {
        let node = self.node_mut(node_index)?;
        ensure!(
            node.pointer(&format!("/extras/{}", lif::EXTRAS_PROP))
                .is_some(),
            "node {node_index} is not a map prop"
        );
        let before = node_transform(node)?;
        let after = NodeTransform {
            translation,
            rotation,
            scale,
        };
        if before == after {
            return Ok(());
        }
        write_node_transform(node, after);
        self.undo.push(DocumentEdit::PropTransform {
            node_index,
            before,
            after,
        });
        self.redo.clear();
        self.dirty = true;
        Ok(())
    }

    /// Updates a prop while an editor gesture is active. Repeated calls for the
    /// same node coalesce into one future undo entry; call
    /// [`Self::commit_pending_prop_transform`] after the gesture ends.
    pub fn update_prop_transform(
        &mut self,
        node_index: usize,
        translation: [f32; 3],
        rotation: [f32; 4],
        scale: [f32; 3],
    ) -> Result<()> {
        let after = NodeTransform {
            translation,
            rotation,
            scale,
        };
        if self
            .pending_prop_transform
            .is_some_and(|edit| edit.node_index != node_index)
        {
            self.commit_pending_prop_transform();
        }
        let before = if let Some(edit) = self.pending_prop_transform {
            edit.before
        } else {
            let before = node_transform(self.node_mut(node_index)?)?;
            self.redo.clear();
            before
        };
        write_node_transform(self.node_mut(node_index)?, after);
        self.pending_prop_transform = Some(PropTransformEdit {
            node_index,
            before,
            after,
        });
        self.dirty = true;
        Ok(())
    }

    /// Commits the current transform gesture as one undoable edit.
    pub fn commit_pending_prop_transform(&mut self) -> bool {
        let Some(edit) = self.pending_prop_transform.take() else {
            return false;
        };
        if edit.before == edit.after {
            return false;
        }
        self.undo.push(DocumentEdit::PropTransform {
            node_index: edit.node_index,
            before: edit.before,
            after: edit.after,
        });
        true
    }

    pub fn has_pending_prop_transform(&self) -> bool {
        self.pending_prop_transform.is_some()
    }

    pub fn set_prop_deleted(&mut self, node_index: usize, deleted: bool) -> Result<()> {
        self.commit_pending_prop_transform();
        let node = self.node_mut(node_index)?;
        ensure!(
            node.pointer(&format!("/extras/{}", lif::EXTRAS_PROP))
                .is_some(),
            "node {node_index} is not a map prop"
        );
        let before = self.deleted_props.contains(&node_index);
        if before == deleted {
            return Ok(());
        }
        if deleted {
            self.deleted_props.insert(node_index);
        } else {
            self.deleted_props.remove(&node_index);
        }
        self.undo.push(DocumentEdit::PropDeleted {
            node_index,
            before,
            after: deleted,
        });
        self.redo.clear();
        self.dirty = true;
        Ok(())
    }

    pub fn prop_deleted(&self, node_index: usize) -> bool {
        self.deleted_props.contains(&node_index)
    }

    /// Duplicates a leaf map prop and places it beside the original. The new
    /// node is intentionally added to the same parent, preserving the GLTF
    /// scene hierarchy used by the runtime.
    pub fn duplicate_prop(&mut self, node_index: usize) -> Result<usize> {
        self.commit_pending_prop_transform();
        let nodes = self
            .root
            .get("nodes")
            .and_then(serde_json::Value::as_array)
            .context("map has no nodes")?;
        let source = nodes
            .get(node_index)
            .cloned()
            .with_context(|| format!("map has no node {node_index}"))?;
        ensure!(
            source
                .pointer(&format!("/extras/{}", lif::EXTRAS_PROP))
                .is_some(),
            "node {node_index} is not a map prop"
        );
        let mut duplicate = source;
        let mut transform = node_transform(&duplicate)?;
        transform.translation[0] += 20.0;
        write_node_transform(&mut duplicate, transform);

        let new_index = nodes.len();
        self.root
            .get_mut("nodes")
            .and_then(serde_json::Value::as_array_mut)
            .expect("nodes validated above")
            .push(duplicate);

        let mut parent_found = false;
        if let Some(nodes) = self
            .root
            .get_mut("nodes")
            .and_then(serde_json::Value::as_array_mut)
        {
            for parent in nodes {
                if let Some(children) = parent
                    .get_mut("children")
                    .and_then(serde_json::Value::as_array_mut)
                    && children
                        .iter()
                        .any(|value| value.as_u64() == Some(node_index as u64))
                {
                    children.push(serde_json::json!(new_index));
                    parent_found = true;
                    break;
                }
            }
        }
        if !parent_found {
            let scenes = self
                .root
                .get_mut("scenes")
                .and_then(serde_json::Value::as_array_mut)
                .context("map has no scenes")?;
            let scene = scenes.first_mut().context("map has no default scene")?;
            scene
                .get_mut("nodes")
                .and_then(serde_json::Value::as_array_mut)
                .context("default scene has no nodes")?
                .push(serde_json::json!(new_index));
        }
        self.scene.props += 1;
        self.dirty = true;
        self.redo.clear();
        Ok(new_index)
    }

    /// Returns a prop node's current document transform for synchronizing an
    /// undo/redo operation back into the live editor scene.
    pub fn prop_transform(&self, node_index: usize) -> Result<([f32; 3], [f32; 4], [f32; 3])> {
        let node = self
            .root
            .get("nodes")
            .and_then(serde_json::Value::as_array)
            .and_then(|nodes| nodes.get(node_index))
            .with_context(|| format!("map has no node {node_index}"))?;
        let transform = node_transform(node)?;
        Ok((transform.translation, transform.rotation, transform.scale))
    }

    fn apply_edit(&mut self, edit: DocumentEdit, forward: bool) {
        match edit {
            DocumentEdit::Collision {
                index,
                before,
                after,
            } => {
                self.collision_cells[index] = if forward { after } else { before };
            }
            DocumentEdit::PropTransform {
                node_index,
                before,
                after,
            } => {
                // A history entry was created only after the node was validated.
                let node = self.node_mut(node_index).expect("history prop node exists");
                write_node_transform(node, if forward { after } else { before });
            }
            DocumentEdit::PropDeleted {
                node_index,
                before,
                after,
            } => {
                if if forward { after } else { before } {
                    self.deleted_props.insert(node_index);
                } else {
                    self.deleted_props.remove(&node_index);
                }
            }
        }
    }

    fn node_mut(&mut self, node_index: usize) -> Result<&mut serde_json::Value> {
        self.root
            .get_mut("nodes")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|nodes| nodes.get_mut(node_index))
            .with_context(|| format!("map has no node {node_index}"))
    }

    /// Atomically writes collision edits back to the source GLB.
    ///
    /// This first save path only changes the fixed-size `LIF_editor` buffer;
    /// terrain/object editing will use a JSON-and-buffer rebuild path later.
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
            pending_prop_transform: None,
            deleted_props: BTreeSet::new(),
            dirty: false,
            root: root.clone(),
            json_chunk,
            bin_chunk,
        })
    }
}

fn node_transform(node: &serde_json::Value) -> Result<NodeTransform> {
    Ok(NodeTransform {
        translation: json_array(node, "translation", [0.0; 3])?,
        rotation: json_array(node, "rotation", [0.0, 0.0, 0.0, 1.0])?,
        scale: json_array(node, "scale", [1.0; 3])?,
    })
}

/// Removes deleted map-prop nodes and rewrites map-scene/node child indices.
/// Map props are leaf nodes, so map GLBs have no skin or animation node refs
/// to preserve here; those belong to the separately referenced model GLBs.
fn compact_deleted_props(
    root: &serde_json::Value,
    deleted: &BTreeSet<usize>,
) -> Result<serde_json::Value> {
    let mut root = root.clone();
    if deleted.is_empty() {
        return Ok(root);
    }
    let nodes = root
        .get("nodes")
        .and_then(serde_json::Value::as_array)
        .context("map has no nodes")?;
    let mut remap = vec![None; nodes.len()];
    let mut kept = Vec::with_capacity(nodes.len() - deleted.len());
    for (old, node) in nodes.iter().enumerate() {
        if deleted.contains(&old) {
            continue;
        }
        remap[old] = Some(kept.len());
        kept.push(node.clone());
    }
    for node in &mut kept {
        remap_index_array(node.get_mut("children"), &remap);
    }
    if let Some(scenes) = root
        .get_mut("scenes")
        .and_then(serde_json::Value::as_array_mut)
    {
        for scene in scenes {
            remap_index_array(scene.get_mut("nodes"), &remap);
        }
    }
    root["nodes"] = serde_json::Value::Array(kept);
    Ok(root)
}

fn remap_index_array(value: Option<&mut serde_json::Value>, remap: &[Option<usize>]) {
    let Some(values) = value.and_then(serde_json::Value::as_array_mut) else {
        return;
    };
    values.retain_mut(|value| {
        let Some(old) = value.as_u64().map(|index| index as usize) else {
            return false;
        };
        let Some(new) = remap.get(old).and_then(|index| *index) else {
            return false;
        };
        *value = serde_json::json!(new);
        true
    });
}

fn json_array<const N: usize>(
    node: &serde_json::Value,
    key: &str,
    default: [f32; N],
) -> Result<[f32; N]> {
    let Some(values) = node.get(key) else {
        return Ok(default);
    };
    let values = values
        .as_array()
        .with_context(|| format!("node {key} is not an array"))?;
    ensure!(
        values.len() == N,
        "node {key} has {} values, expected {N}",
        values.len()
    );
    let mut result = default;
    for (index, value) in values.iter().enumerate() {
        result[index] = value
            .as_f64()
            .map(|value| value as f32)
            .with_context(|| format!("node {key}[{index}] is not a number"))?;
    }
    Ok(result)
}

fn write_node_transform(node: &mut serde_json::Value, transform: NodeTransform) {
    node["translation"] = serde_json::json!(transform.translation);
    node["rotation"] = serde_json::json!(transform.rotation);
    node["scale"] = serde_json::json!(transform.scale);
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

    #[test]
    fn prop_transform_is_written_to_the_glb_json() {
        let directory = write_document(root(lif::FORMAT_VERSION));
        let path = directory.path().join("map.glb");
        let mut document = MapDocument::open(&path).expect("document");

        document
            .set_prop_transform(1, [1.0, 2.0, 3.0], [0.0, 0.0, 0.0, 1.0], [2.0; 3])
            .expect("prop transform");
        document.save().expect("save");

        let saved = MapDocument::open(path).expect("reopen");
        assert_eq!(
            saved.root.pointer("/nodes/1/translation"),
            Some(&json!([1.0, 2.0, 3.0]))
        );
        assert_eq!(
            saved.root.pointer("/nodes/1/scale"),
            Some(&json!([2.0, 2.0, 2.0]))
        );
    }

    #[test]
    fn prop_transform_edits_participate_in_undo_and_redo() {
        let directory = write_document(root(lif::FORMAT_VERSION));
        let mut document = MapDocument::open(directory.path().join("map.glb")).expect("document");

        document
            .set_prop_transform(1, [4.0, 5.0, 6.0], [0.0, 0.0, 0.0, 1.0], [1.0; 3])
            .expect("edit prop");
        assert!(document.undo());
        assert_eq!(
            document.root.pointer("/nodes/1/translation"),
            Some(&json!([0.0, 0.0, 0.0]))
        );
        assert!(document.redo());
        assert_eq!(
            document.root.pointer("/nodes/1/translation"),
            Some(&json!([4.0, 5.0, 6.0]))
        );
    }

    #[test]
    fn a_prop_transform_gesture_is_one_undo_entry_and_branches_history() {
        let directory = write_document(root(lif::FORMAT_VERSION));
        let mut document = MapDocument::open(directory.path().join("map.glb")).expect("document");

        document
            .update_prop_transform(1, [1.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0], [1.0; 3])
            .expect("first gesture update");
        document
            .update_prop_transform(1, [2.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0], [1.0; 3])
            .expect("second gesture update");
        assert!(document.commit_pending_prop_transform());
        assert!(document.undo());
        assert_eq!(
            document.root.pointer("/nodes/1/translation"),
            Some(&json!([0.0, 0.0, 0.0]))
        );

        document
            .update_prop_transform(1, [3.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0], [1.0; 3])
            .expect("branch update");
        document.commit_pending_prop_transform();
        assert!(!document.redo(), "a new edit must discard the redo branch");
    }
}
