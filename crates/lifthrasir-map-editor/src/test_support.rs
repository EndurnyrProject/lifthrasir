//! Small, synthetic GLB fixture for headless application tests (no retail data).

use lifthrasir_data::lif::{self, LifCollisionCell, LifCollisionFlags};
use lifthrasir_map_editor::MapDocument;
use serde_json::json;

pub(crate) fn document() -> (tempfile::TempDir, MapDocument) {
    let directory = tempfile::tempdir().unwrap();
    let bin = lif::LifCollision::encode(
        &[LifCollisionCell {
            height: [0.0; 4],
            flags: LifCollisionFlags::WALKABLE,
        }],
        1,
        1,
    )
    .unwrap();
    let root = json!({
        "extensions": {
            "LIF_map": {
                "format_version": lif::FORMAT_VERSION,
                "rsw_hash": "r", "gnd_hash": "g", "gat_hash": "a",
                "ambient_color": [0.0, 0.0, 0.0]
            },
            "LIF_editor": {"collision": {"width": 1, "height": 1, "buffer_view": 0}}
        },
        "nodes": [{"extras": {"lif_prop": {"model": "ro://models/tree.glb"}}}],
        "scenes": [{"nodes": [0]}],
        "bufferViews": [{"byteOffset": 0, "byteLength": bin.len()}]
    });
    let mut json = serde_json::to_vec(&root).unwrap();
    json.resize(json.len().next_multiple_of(4), b' ');
    let mut bin = bin;
    bin.resize(bin.len().next_multiple_of(4), 0);
    let mut bytes = b"glTF".to_vec();
    bytes.extend_from_slice(&2u32.to_le_bytes());
    bytes.extend_from_slice(&((28 + json.len() + bin.len()) as u32).to_le_bytes());
    bytes.extend_from_slice(&(json.len() as u32).to_le_bytes());
    bytes.extend_from_slice(b"JSON");
    bytes.extend_from_slice(&json);
    bytes.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    bytes.extend_from_slice(b"BIN\0");
    bytes.extend_from_slice(&bin);
    let path = directory.path().join("map.glb");
    std::fs::write(&path, bytes).unwrap();
    let document = MapDocument::open(path).unwrap();
    (directory, document)
}
