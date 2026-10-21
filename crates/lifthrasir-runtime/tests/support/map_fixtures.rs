use bevy::prelude::Resource;
use lifthrasir_data::lif::{self, LifCollision, LifEditor, LifGat};
use std::{borrow::Cow, path::Path};

/// Adapts the committed v4 corpus to v5 in a temporary asset root. Never
/// modifies source fixtures or adds legacy-format support to the runtime.
#[derive(Resource)]
pub struct StagedFixtures(tempfile::TempDir);

impl StagedFixtures {
    pub fn new(source: &Path) -> Self {
        let staged = Self(tempfile::tempdir().unwrap());
        copy_directory(source, staged.path());
        staged
    }

    pub fn path(&self) -> &Path {
        self.0.path()
    }
}

fn copy_directory(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let output = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_directory(&path, &output);
        } else if path.extension().is_some_and(|extension| extension == "glb") {
            std::fs::write(output, current_map_fixture(&std::fs::read(&path).unwrap())).unwrap();
        } else {
            std::fs::copy(path, output).unwrap();
        }
    }
}

fn current_map_fixture(bytes: &[u8]) -> Vec<u8> {
    // A later schema must update this adapter deliberately, not merely relabel
    // old fixture data as whatever FORMAT_VERSION happens to be.
    assert_eq!(
        lif::FORMAT_VERSION,
        5,
        "update the test fixture adapter for the new schema"
    );
    let mut glb = gltf::binary::Glb::from_slice(bytes).unwrap();
    let mut root: serde_json::Value = serde_json::from_slice(&glb.json).unwrap();
    let Some(version) = root
        .pointer("/extensions/LIF_map/format_version")
        .and_then(serde_json::Value::as_u64)
    else {
        return bytes.to_vec();
    };
    if version == 5 {
        // The v4 corpus's intentionally unsupported fixture was version 5.
        assert!(root.pointer("/extensions/LIF_editor").is_none());
        root["extensions"][lif::EXTENSION_MAP]["format_version"] = (lif::FORMAT_VERSION + 1).into();
        glb.json = Cow::Owned(serde_json::to_vec(&root).unwrap());
        return glb.to_vec().unwrap();
    }
    assert_eq!(version, 4, "unexpected committed map fixture version");
    let gat: LifGat =
        serde_json::from_value(root["extensions"][lif::EXTENSION_GAT].clone()).unwrap();
    let view_index = gat.buffer_view as usize;
    let view = &root["bufferViews"][view_index];
    let start = view
        .get("byteOffset")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0) as usize;
    let length = view["byteLength"].as_u64().unwrap() as usize;
    let mut bin = glb.bin.as_ref().unwrap().to_vec();
    let raw = &bin[start..start + length];
    gat.validate(raw).unwrap();
    let altitude = LifGat::decode(raw).unwrap();
    let (mut collision, encoded) = LifCollision::from_altitude(&altitude).unwrap();
    assert!(encoded.len() <= length);
    bin[start..start + length].fill(0);
    bin[start..start + encoded.len()].copy_from_slice(&encoded);
    collision.buffer_view = view_index;
    root["bufferViews"][view_index]["byteLength"] = encoded.len().into();
    root["extensions"]
        .as_object_mut()
        .unwrap()
        .remove(lif::EXTENSION_GAT);
    root["extensions"][lif::EXTENSION_EDITOR] =
        serde_json::to_value(LifEditor { collision }).unwrap();
    root["extensions"][lif::EXTENSION_MAP]["format_version"] = lif::FORMAT_VERSION.into();
    for extension in root["extensionsUsed"].as_array_mut().unwrap() {
        if extension == lif::EXTENSION_GAT {
            *extension = lif::EXTENSION_EDITOR.into();
        }
    }
    glb.json = Cow::Owned(serde_json::to_vec(&root).unwrap());
    glb.bin = Some(Cow::Owned(bin));
    glb.to_vec().unwrap()
}
