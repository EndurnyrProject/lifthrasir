//! Format-agnostic glb core shared by the map and model converters:
//! the bin-chunk/accessor plumbing, geometry-primitive and image/texture
//! assembly, the GLB container, the runtime root-fix rotation and its
//! coordinate helpers, small path/hash utilities, and the re-read helpers
//! both validators lean on.
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

use crate::converters::map::textures::TextureOut;
use anyhow::{Context, bail, ensure};
use glam::{Quat, Vec3};
use gltf_json as json;
use json::validation::{Checked::Valid, USize64};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;

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

/// Vertex data for one mesh primitive, already in glTF space -- callers apply
/// `to_gltf_vec` (or not, for raw-local model geometry) before handing it over.
pub struct GeometryAttributes<'a> {
    pub positions: &'a [Vec3],
    pub normals: &'a [Vec3],
    pub colors: Option<&'a [[f32; 4]]>,
    pub uvs: &'a [[f32; 2]],
    pub uv1: Option<&'a [[f32; 2]]>,
    pub indices: &'a [u32],
}

/// Push one triangle primitive's views and accessors (positions with min/max
/// bounds, normals, optional vertex colors, uvs, indices) and return the
/// assembled `Primitive`. `label` names the primitive in error messages.
pub fn push_geometry_primitive(
    root: &mut json::Root,
    bin: &mut BinChunk,
    label: &str,
    geometry: &GeometryAttributes,
    material: json::Index<json::Material>,
) -> anyhow::Result<json::mesh::Primitive> {
    let count = geometry.positions.len();
    if geometry.normals.len() != count
        || geometry.uvs.len() != count
        || geometry.uv1.is_some_and(|uv1| uv1.len() != count)
        || geometry.colors.is_some_and(|colors| colors.len() != count)
    {
        bail!(
            "{label} has mismatched attribute counts: {count} positions, {} normals, {} colors, {} uvs",
            geometry.normals.len(),
            geometry.colors.map_or(count, <[_]>::len),
            geometry.uvs.len()
        );
    }

    let (min, max) = bounds(geometry.positions);
    let positions_view = bin.push_view(
        &f32_bytes(geometry.positions.iter().flat_map(|p| p.to_array())),
        Some(json::buffer::Target::ArrayBuffer),
    );
    let mut positions_accessor = accessor(
        positions_view,
        count,
        json::accessor::ComponentType::F32,
        json::accessor::Type::Vec3,
    );
    positions_accessor.min = Some(serde_json::json!(min.to_array()));
    positions_accessor.max = Some(serde_json::json!(max.to_array()));
    let positions_accessor = json::Index::push(&mut root.accessors, positions_accessor);

    let normals_view = bin.push_view(
        &f32_bytes(geometry.normals.iter().flat_map(|n| n.to_array())),
        Some(json::buffer::Target::ArrayBuffer),
    );
    let normals_accessor = json::Index::push(
        &mut root.accessors,
        accessor(
            normals_view,
            count,
            json::accessor::ComponentType::F32,
            json::accessor::Type::Vec3,
        ),
    );

    let colors_accessor = geometry.colors.map(|colors| {
        let view = bin.push_view(
            &f32_bytes(colors.iter().flatten().copied()),
            Some(json::buffer::Target::ArrayBuffer),
        );
        json::Index::push(
            &mut root.accessors,
            accessor(
                view,
                count,
                json::accessor::ComponentType::F32,
                json::accessor::Type::Vec4,
            ),
        )
    });

    let uvs_view = bin.push_view(
        &f32_bytes(geometry.uvs.iter().flatten().copied()),
        Some(json::buffer::Target::ArrayBuffer),
    );
    let uvs_accessor = json::Index::push(
        &mut root.accessors,
        accessor(
            uvs_view,
            count,
            json::accessor::ComponentType::F32,
            json::accessor::Type::Vec2,
        ),
    );

    let index_bytes: Vec<u8> = geometry
        .indices
        .iter()
        .flat_map(|i| i.to_le_bytes())
        .collect();
    let indices_view = bin.push_view(&index_bytes, Some(json::buffer::Target::ElementArrayBuffer));
    let indices_accessor = json::Index::push(
        &mut root.accessors,
        accessor(
            indices_view,
            geometry.indices.len(),
            json::accessor::ComponentType::U32,
            json::accessor::Type::Scalar,
        ),
    );

    let mut attributes = BTreeMap::new();
    attributes.insert(Valid(json::mesh::Semantic::Positions), positions_accessor);
    attributes.insert(Valid(json::mesh::Semantic::Normals), normals_accessor);
    if let Some(colors_accessor) = colors_accessor {
        attributes.insert(Valid(json::mesh::Semantic::Colors(0)), colors_accessor);
    }
    attributes.insert(Valid(json::mesh::Semantic::TexCoords(0)), uvs_accessor);
    if let Some(uv1) = geometry.uv1 {
        let view = bin.push_view(
            &f32_bytes(uv1.iter().flatten().copied()),
            Some(json::buffer::Target::ArrayBuffer),
        );
        let accessor = json::Index::push(
            &mut root.accessors,
            accessor(
                view,
                count,
                json::accessor::ComponentType::F32,
                json::accessor::Type::Vec2,
            ),
        );
        attributes.insert(Valid(json::mesh::Semantic::TexCoords(1)), accessor);
    }

    Ok(json::mesh::Primitive {
        attributes,
        indices: Some(indices_accessor),
        material: Some(material),
        mode: Valid(json::mesh::Mode::Triangles),
        targets: None,
        extensions: None,
        extras: Default::default(),
    })
}

/// Push one exported texture's image and texture entries and return the
/// `texture::Info` a material's `base_color_texture` wants.
pub fn push_image_and_texture(
    root: &mut json::Root,
    texture: &TextureOut,
    sampler: Option<json::Index<json::texture::Sampler>>,
) -> json::texture::Info {
    let image = json::Index::push(
        &mut root.images,
        json::Image {
            buffer_view: None,
            mime_type: Some(json::image::MimeType("image/png".to_string())),
            uri: Some(texture.relative_path.clone()),
            name: Some(texture.source_name.clone()),
            extensions: None,
            extras: Default::default(),
        },
    );
    let gltf_texture = json::Index::push(
        &mut root.textures,
        json::Texture {
            sampler,
            source: image,
            name: Some(texture.source_name.clone()),
            extensions: None,
            extras: Default::default(),
        },
    );

    json::texture::Info {
        index: gltf_texture,
        tex_coord: 0,
        extensions: None,
        extras: Default::default(),
    }
}

/// Tolerance for values that survive a f32 encode/decode and a quaternion
/// round trip.
pub const EPSILON: f32 = 1e-4;

pub fn ensure_close(label: &str, actual: Vec3, expected: Vec3) -> anyhow::Result<()> {
    ensure!(
        (actual - expected).length() < EPSILON,
        "{label}: expected {expected:?}, got {actual:?}"
    );
    Ok(())
}

/// The single root node every Lifthrasir glb hangs its scene under.
pub fn scene_root(document: &gltf::Document) -> anyhow::Result<gltf::Node<'_>> {
    let scene = document
        .default_scene()
        .context("glb has no default scene")?;
    let roots: Vec<gltf::Node> = scene.nodes().collect();
    ensure!(
        roots.len() == 1,
        "glb scene must have exactly one root node, found {}",
        roots.len()
    );
    Ok(roots.into_iter().next().expect("checked length"))
}

pub fn root_extension<T: DeserializeOwned>(root: &json::Root, key: &str) -> anyhow::Result<T> {
    let value = root
        .extensions
        .as_ref()
        .and_then(|extensions| extensions.others.get(key))
        .with_context(|| format!("glb has no {key} root extension"))?;
    serde_json::from_value(value.clone()).with_context(|| format!("decoding {key}"))
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
