//! Format-agnostic glb assembly core shared by the map and model writers:
//! the bin-chunk/accessor plumbing, the GLB container, the runtime root-fix
//! rotation and its coordinate helpers, and small path/hash utilities.
//!
//! # Coordinate convention
//!
//! The runtime's world is -Y-up; glTF is Y-up. The runtime spawns glb scenes
//! under a single root fix of 180 degrees about X, so every position, normal
//! and node transform written by a caller of this module is pre-rotated by
//! that same rotation -- which is its own inverse, so `(x, y, z)` is stored
//! as `(x, -y, -z)` and a node rotation `q` is stored as `FIX * q`. Applying
//! the runtime root fix to the imported data therefore reproduces the
//! native path's world values exactly, and because the fix is a proper
//! rotation the glb is also right way up in a stock glTF viewer.
//!
//! This assumes Bevy's experimental `GltfLoaderSettings::convert_coordinates`
//! stays at its default (off); enabling it would add a second rotation.

use anyhow::Context;
use glam::{Quat, Vec3};
use gltf_json as json;
use json::validation::{Checked::Valid, USize64};
use serde::Serialize;

/// The runtime root fix: 180 degrees about X, mapping glTF Y-up onto the
/// engine's -Y-up world. Written in exact components (`Quat::from_rotation_x`
/// leaves a 1e-7 residue in the Z term) and self-inverse, so the converter
/// applies the very same rotation to go the other way.
pub const ROOT_FIX: Quat = Quat::from_xyzw(1.0, 0.0, 0.0, 0.0);

/// Native world position -> glTF position; `ROOT_FIX * v`, spelled out so the
/// round trip is bit-exact.
pub fn to_gltf_vec(v: Vec3) -> Vec3 {
    Vec3::new(v.x, -v.y, -v.z)
}

/// Native world rotation -> glTF node rotation.
pub fn to_gltf_quat(q: Quat) -> Quat {
    ROOT_FIX.inverse() * q
}

/// GRF paths are backslash-separated; the `ro://` namespace is not.
pub fn to_forward_slashes(path: &str) -> String {
    path.replace('\\', "/")
}

/// Bin chunk under construction: raw bytes plus the bufferViews addressing
/// them. Every view starts 4-byte aligned.
#[derive(Default)]
pub struct BinChunk {
    pub data: Vec<u8>,
    pub views: Vec<json::buffer::View>,
}

impl BinChunk {
    pub fn push_view(
        &mut self,
        bytes: &[u8],
        target: Option<json::buffer::Target>,
    ) -> json::Index<json::buffer::View> {
        while !self.data.len().is_multiple_of(4) {
            self.data.push(0);
        }
        let offset = self.data.len();
        self.data.extend_from_slice(bytes);

        let view = json::buffer::View {
            buffer: json::Index::new(0),
            byte_length: USize64::from(bytes.len()),
            byte_offset: Some(USize64::from(offset)),
            byte_stride: None,
            name: None,
            target: target.map(Valid),
            extensions: None,
            extras: Default::default(),
        };
        json::Index::push(&mut self.views, view)
    }
}

pub fn f32_bytes(values: impl IntoIterator<Item = f32>) -> Vec<u8> {
    values.into_iter().flat_map(f32::to_le_bytes).collect()
}

pub fn accessor(
    view: json::Index<json::buffer::View>,
    count: usize,
    component_type: json::accessor::ComponentType,
    type_: json::accessor::Type,
) -> json::Accessor {
    json::Accessor {
        buffer_view: Some(view),
        byte_offset: Some(USize64(0)),
        count: USize64::from(count),
        component_type: Valid(json::accessor::GenericComponentType(component_type)),
        type_: Valid(type_),
        min: None,
        max: None,
        name: None,
        normalized: false,
        sparse: None,
        extensions: None,
        extras: Default::default(),
    }
}

pub fn bounds(positions: &[Vec3]) -> (Vec3, Vec3) {
    positions.iter().fold(
        (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
        |(min, max), p| (min.min(*p), max.max(*p)),
    )
}

pub fn extras_for<T: Serialize>(key: &str, value: &T) -> anyhow::Result<json::Extras> {
    let mut map = serde_json::Map::new();
    map.insert(key.to_string(), serde_json::to_value(value)?);
    let raw = serde_json::value::RawValue::from_string(serde_json::to_string(&map)?)
        .context("encoding node extras")?;
    Ok(Some(raw))
}

pub fn hash_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

const GLB_MAGIC: u32 = 0x4654_6C67;
const GLB_VERSION: u32 = 2;
const CHUNK_TYPE_JSON: u32 = 0x4E4F_534A;
const CHUNK_TYPE_BIN: u32 = 0x004E_4942;

/// Wrap the JSON and binary payloads in the GLB container. Both chunks are
/// padded to a 4-byte boundary, JSON with spaces and BIN with zeroes, as the
/// spec requires.
pub fn glb_container(json_bytes: &[u8], bin: &[u8]) -> Vec<u8> {
    let json_padding = (4 - json_bytes.len() % 4) % 4;
    let bin_padding = (4 - bin.len() % 4) % 4;
    let json_len = json_bytes.len() + json_padding;
    let bin_len = bin.len() + bin_padding;

    let total = 12 + 8 + json_len + if bin_len == 0 { 0 } else { 8 + bin_len };
    let mut out = Vec::with_capacity(total);

    out.extend_from_slice(&GLB_MAGIC.to_le_bytes());
    out.extend_from_slice(&GLB_VERSION.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());

    out.extend_from_slice(&(json_len as u32).to_le_bytes());
    out.extend_from_slice(&CHUNK_TYPE_JSON.to_le_bytes());
    out.extend_from_slice(json_bytes);
    out.extend(std::iter::repeat_n(b' ', json_padding));

    if bin_len > 0 {
        out.extend_from_slice(&(bin_len as u32).to_le_bytes());
        out.extend_from_slice(&CHUNK_TYPE_BIN.to_le_bytes());
        out.extend_from_slice(bin);
        out.extend(std::iter::repeat_n(0u8, bin_padding));
    }

    out
}
