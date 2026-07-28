use crate::string_utils::parse_korean_string;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

const MAX_STRING_LENGTH: i32 = 1024;
const MAX_RECORDS: i32 = 1_000_000;
const MAX_GLOBAL_TEXTURES: i32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rsm2Version {
    V2_2,
    V2_3,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rsm2 {
    pub version: Rsm2Version,
    pub animation_length: i32,
    pub shade_type: i32,
    pub alpha: u8,
    pub frames_per_second: f32,
    pub global_textures: Vec<String>,
    pub roots: Vec<String>,
    pub nodes: Vec<Rsm2Node>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rsm2Node {
    pub name: String,
    pub parent_name: String,
    pub textures: Rsm2NodeTextures,
    pub offset_matrix: [f32; 9],
    pub offset_position: [f32; 3],
    pub vertices: Vec<[f32; 3]>,
    pub texture_vertices: Vec<Rsm2TextureVertex>,
    pub faces: Vec<Rsm2Face>,
    pub scale_keys: Vec<Rsm2ScaleKey>,
    pub rotation_keys: Vec<Rsm2RotationKey>,
    pub position_keys: Vec<Rsm2PositionKey>,
    pub texture_animations: Vec<Rsm2TextureAnimation>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Rsm2NodeTextures {
    GlobalIndices(Vec<usize>),
    Names(Vec<String>),
}

impl Rsm2NodeTextures {
    pub fn len(&self) -> usize {
        match self {
            Self::GlobalIndices(values) => values.len(),
            Self::Names(values) => values.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rsm2TextureVertex {
    pub unknown: f32,
    pub coordinates: [f32; 2],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rsm2Face {
    pub record_length: i32,
    pub vertex_indices: [usize; 3],
    pub texture_vertex_indices: [usize; 3],
    pub texture_index: usize,
    pub padding: i16,
    pub two_sided: i32,
    pub smooth_groups: Vec<i32>,
    pub unknown_words: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rsm2ScaleKey {
    pub time: i32,
    pub scale: [f32; 3],
    pub unknown: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rsm2RotationKey {
    pub time: i32,
    pub quaternion_xyzw: [f32; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rsm2PositionKey {
    pub time: i32,
    pub position: [f32; 3],
    pub unknown: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rsm2TextureAnimation {
    pub texture_index: usize,
    pub channels: Vec<Rsm2TextureChannel>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rsm2TextureChannel {
    pub channel_type: Rsm2TextureChannelType,
    pub keys: Vec<Rsm2TextureKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rsm2TextureChannelType {
    TranslateU,
    TranslateV,
    ScaleU,
    ScaleV,
    Rotate,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rsm2TextureKey {
    pub time: i32,
    pub value: f32,
}

#[derive(Error, Debug, Clone, PartialEq)]
pub enum Rsm2Error {
    #[error("truncated {field} at byte {offset}: needed {needed} bytes, found {remaining}")]
    Truncated {
        field: String,
        offset: usize,
        needed: usize,
        remaining: usize,
    },
    #[error("invalid RSM2 magic {0:?}")]
    InvalidMagic([u8; 4]),
    #[error("unsupported RSM2 version {major}.{minor}")]
    UnsupportedVersion { major: u8, minor: u8 },
    #[error("invalid count for {field}: {value} (maximum {maximum})")]
    InvalidCount {
        field: String,
        value: i32,
        maximum: i32,
    },
    #[error("invalid dynamic string length for {field}: {length}")]
    InvalidStringLength { field: String, length: i32 },
    #[error("invalid empty string for {field}")]
    InvalidString { field: String },
    #[error("invalid face record length {length} in node {node} face {face}")]
    InvalidFaceLength {
        node: String,
        face: usize,
        length: i32,
    },
    #[error("non-finite value in {field}")]
    NonFinite { field: String },
    #[error("non-increasing key time in {field} at key {index}: {previous} then {current}")]
    NonIncreasingKey {
        field: String,
        index: usize,
        previous: i32,
        current: i32,
    },
    #[error("{field} index {index} is out of range for length {length}")]
    IndexOutOfRange {
        field: String,
        index: i64,
        length: usize,
    },
    #[error("unknown texture animation channel type {value} in {field}")]
    UnknownTextureChannel { field: String, value: i32 },
    #[error("duplicate node name {name:?}")]
    DuplicateNode { name: String },
    #[error("duplicate declared root {name:?}")]
    DuplicateRoot { name: String },
    #[error("declared root {name:?} is not a root node")]
    MissingRoot { name: String },
    #[error("node {node:?} references missing parent {parent:?}")]
    MissingParent { node: String, parent: String },
    #[error("node {name:?} is an undeclared root or is unreachable from a declared root")]
    OrphanNode { name: String },
    #[error("node hierarchy contains a cycle through {name:?}")]
    NodeCycle { name: String },
    #[error("volume records are unsupported (declared count {count})")]
    UnsupportedVolumes { count: i32 },
    #[error("{count} trailing bytes after RSM2 payload")]
    TrailingBytes { count: usize },
}

impl Rsm2 {
    pub fn from_bytes(data: &[u8]) -> Result<Self, Rsm2Error> {
        let mut cursor = Cursor::new(data);
        let magic = cursor.array::<4>("magic")?;
        if magic != *b"GRSM" {
            return Err(Rsm2Error::InvalidMagic(magic));
        }

        let major = cursor.u8("version major")?;
        let minor = cursor.u8("version minor")?;
        let version = match (major, minor) {
            (2, 2) => Rsm2Version::V2_2,
            (2, 3) => Rsm2Version::V2_3,
            _ => return Err(Rsm2Error::UnsupportedVersion { major, minor }),
        };
        let animation_length = cursor.i32("animation length")?;
        let shade_type = cursor.i32("shade type")?;
        let alpha = cursor.u8("alpha")?;
        let frames_per_second = cursor.f32("frames per second")?;
        finite(frames_per_second, "frames per second")?;

        let global_textures = if version == Rsm2Version::V2_2 {
            let count = cursor.count("global texture count", MAX_GLOBAL_TEXTURES)?;
            (0..count)
                .map(|index| cursor.dynamic_string(&format!("global texture {index}"), false))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };

        let root_count = cursor.count("root count", MAX_RECORDS)?;
        let roots = (0..root_count)
            .map(|index| cursor.dynamic_string(&format!("root {index}"), true))
            .collect::<Result<Vec<_>, _>>()?;
        let node_count = cursor.count("node count", MAX_RECORDS)?;
        let mut nodes = Vec::new();
        for index in 0..node_count {
            nodes.push(parse_node(
                &mut cursor,
                version,
                global_textures.len(),
                index,
            )?);
        }

        match cursor.remaining() {
            0 => {}
            1..=3 => {
                return Err(Rsm2Error::Truncated {
                    field: "volume count".to_owned(),
                    offset: cursor.offset,
                    needed: 4,
                    remaining: cursor.remaining(),
                });
            }
            _ => {
                let volume_count = cursor.count("volume count", MAX_RECORDS)? as i32;
                if volume_count != 0 {
                    return Err(Rsm2Error::UnsupportedVolumes {
                        count: volume_count,
                    });
                }
                if cursor.remaining() != 0 {
                    return Err(Rsm2Error::TrailingBytes {
                        count: cursor.remaining(),
                    });
                }
            }
        }

        validate_hierarchy(&roots, &nodes)?;
        Ok(Self {
            version,
            animation_length,
            shade_type,
            alpha,
            frames_per_second,
            global_textures,
            roots,
            nodes,
        })
    }
}

fn parse_node(
    cursor: &mut Cursor<'_>,
    version: Rsm2Version,
    global_texture_count: usize,
    node_index: usize,
) -> Result<Rsm2Node, Rsm2Error> {
    let name = cursor.dynamic_string(&format!("node {node_index} name"), true)?;
    let parent_name = cursor.dynamic_string(&format!("node {name} parent"), false)?;
    let texture_count = cursor.count(&format!("node {name} texture count"), MAX_RECORDS)?;
    let textures = match version {
        Rsm2Version::V2_2 => {
            let mut indices = Vec::new();
            for index in 0..texture_count {
                indices.push(cursor.index(
                    &format!("node {name} texture {index}"),
                    global_texture_count,
                )?);
            }
            Rsm2NodeTextures::GlobalIndices(indices)
        }
        Rsm2Version::V2_3 => Rsm2NodeTextures::Names(
            (0..texture_count)
                .map(|index| cursor.dynamic_string(&format!("node {name} texture {index}"), false))
                .collect::<Result<Vec<_>, _>>()?,
        ),
    };

    let offset_matrix = cursor.f32_array(&format!("node {name} offset matrix"))?;
    let offset_position = cursor.f32_array(&format!("node {name} offset position"))?;
    let vertex_count = cursor.count(&format!("node {name} vertex count"), MAX_RECORDS)?;
    let mut vertices = Vec::new();
    for index in 0..vertex_count {
        vertices.push(cursor.f32_array(&format!("node {name} vertex {index}"))?);
    }

    let texture_vertex_count =
        cursor.count(&format!("node {name} texture vertex count"), MAX_RECORDS)?;
    let mut texture_vertices = Vec::new();
    for index in 0..texture_vertex_count {
        let unknown = cursor.f32(&format!("node {name} texture vertex {index} unknown"))?;
        finite(
            unknown,
            &format!("node {name} texture vertex {index} unknown"),
        )?;
        let coordinates =
            cursor.f32_array(&format!("node {name} texture vertex {index} coordinates"))?;
        texture_vertices.push(Rsm2TextureVertex {
            unknown,
            coordinates,
        });
    }

    let face_count = cursor.count(&format!("node {name} face count"), MAX_RECORDS)?;
    let mut faces = Vec::new();
    for face_index in 0..face_count {
        faces.push(parse_face(
            cursor,
            &name,
            face_index,
            vertex_count,
            texture_vertex_count,
            textures.len(),
        )?);
    }

    let scale_keys = parse_keys(
        cursor,
        &format!("node {name} scale keys"),
        |cursor, field| {
            Ok(Rsm2ScaleKey {
                time: cursor.i32(&format!("{field} time"))?,
                scale: cursor.f32_array(&format!("{field} value"))?,
                unknown: {
                    let value = cursor.f32(&format!("{field} unknown"))?;
                    finite(value, &format!("{field} unknown"))?;
                    value
                },
            })
        },
        |key| key.time,
    )?;
    let rotation_keys = parse_keys(
        cursor,
        &format!("node {name} rotation keys"),
        |cursor, field| {
            Ok(Rsm2RotationKey {
                time: cursor.i32(&format!("{field} time"))?,
                quaternion_xyzw: cursor.f32_array(&format!("{field} value"))?,
            })
        },
        |key| key.time,
    )?;
    let position_keys = parse_keys(
        cursor,
        &format!("node {name} position keys"),
        |cursor, field| {
            Ok(Rsm2PositionKey {
                time: cursor.i32(&format!("{field} time"))?,
                position: cursor.f32_array(&format!("{field} value"))?,
                unknown: {
                    let value = cursor.f32(&format!("{field} unknown"))?;
                    finite(value, &format!("{field} unknown"))?;
                    value
                },
            })
        },
        |key| key.time,
    )?;

    let texture_animations = if version == Rsm2Version::V2_3 {
        parse_texture_animations(cursor, &name, textures.len())?
    } else {
        Vec::new()
    };

    Ok(Rsm2Node {
        name,
        parent_name,
        textures,
        offset_matrix,
        offset_position,
        vertices,
        texture_vertices,
        faces,
        scale_keys,
        rotation_keys,
        position_keys,
        texture_animations,
    })
}

fn parse_face(
    cursor: &mut Cursor<'_>,
    node: &str,
    face: usize,
    vertex_count: usize,
    texture_vertex_count: usize,
    texture_count: usize,
) -> Result<Rsm2Face, Rsm2Error> {
    let length = cursor.i32(&format!("node {node} face {face} length"))?;
    if length < 24 || length % 4 != 0 {
        return Err(Rsm2Error::InvalidFaceLength {
            node: node.to_owned(),
            face,
            length,
        });
    }
    if length as usize > cursor.remaining() {
        return Err(Rsm2Error::Truncated {
            field: format!("node {node} face {face} record"),
            offset: cursor.offset,
            needed: length as usize,
            remaining: cursor.remaining(),
        });
    }
    let vertex_indices =
        cursor.u16_indices(&format!("node {node} face {face} vertex"), vertex_count)?;
    let texture_vertex_indices = cursor.u16_indices(
        &format!("node {node} face {face} texture vertex"),
        texture_vertex_count,
    )?;
    let texture_index =
        cursor.i16_index(&format!("node {node} face {face} texture"), texture_count)?;
    let padding = cursor.i16(&format!("node {node} face {face} padding"))?;
    let two_sided = cursor.i32(&format!("node {node} face {face} two sided"))?;
    let extra_word_count = (length as usize - 20) / 4;
    let smooth_group_count = extra_word_count.min(3);
    let mut smooth_groups = Vec::new();
    for index in 0..smooth_group_count {
        smooth_groups.push(cursor.i32(&format!("node {node} face {face} smooth group {index}"))?);
    }
    let mut unknown_words = Vec::new();
    for index in smooth_group_count..extra_word_count {
        unknown_words.push(cursor.i32(&format!("node {node} face {face} unknown word {index}"))?);
    }
    Ok(Rsm2Face {
        record_length: length,
        vertex_indices,
        texture_vertex_indices,
        texture_index,
        padding,
        two_sided,
        smooth_groups,
        unknown_words,
    })
}

fn parse_keys<T>(
    cursor: &mut Cursor<'_>,
    field: &str,
    mut parse: impl FnMut(&mut Cursor<'_>, &str) -> Result<T, Rsm2Error>,
    time: impl Fn(&T) -> i32,
) -> Result<Vec<T>, Rsm2Error> {
    let count = cursor.count(&format!("{field} count"), MAX_RECORDS)?;
    let mut keys = Vec::new();
    for index in 0..count {
        let key = parse(cursor, &format!("{field} key {index}"))?;
        if let Some(previous) = keys.last() {
            let previous_time = time(previous);
            let current_time = time(&key);
            if current_time <= previous_time {
                return Err(Rsm2Error::NonIncreasingKey {
                    field: field.to_owned(),
                    index,
                    previous: previous_time,
                    current: current_time,
                });
            }
        }
        keys.push(key);
    }
    Ok(keys)
}

fn parse_texture_animations(
    cursor: &mut Cursor<'_>,
    node: &str,
    texture_count: usize,
) -> Result<Vec<Rsm2TextureAnimation>, Rsm2Error> {
    let count = cursor.count(&format!("node {node} texture animation count"), MAX_RECORDS)?;
    let mut animations = Vec::new();
    for animation_index in 0..count {
        let field = format!("node {node} texture animation {animation_index}");
        let texture_index = cursor.index(&format!("{field} texture"), texture_count)?;
        let channel_count = cursor.count(&format!("{field} channel count"), MAX_RECORDS)?;
        let mut channels = Vec::new();
        for channel_index in 0..channel_count {
            let channel_field = format!("{field} channel {channel_index}");
            let value = cursor.i32(&format!("{channel_field} type"))?;
            let channel_type = match value {
                0 => Rsm2TextureChannelType::TranslateU,
                1 => Rsm2TextureChannelType::TranslateV,
                2 => Rsm2TextureChannelType::ScaleU,
                3 => Rsm2TextureChannelType::ScaleV,
                4 => Rsm2TextureChannelType::Rotate,
                _ => {
                    return Err(Rsm2Error::UnknownTextureChannel {
                        field: channel_field,
                        value,
                    });
                }
            };
            let keys = parse_keys(
                cursor,
                &format!("{channel_field} keys"),
                |cursor, key_field| {
                    let time = cursor.i32(&format!("{key_field} time"))?;
                    let value = cursor.f32(&format!("{key_field} value"))?;
                    finite(value, &format!("{key_field} value"))?;
                    Ok(Rsm2TextureKey { time, value })
                },
                |key| key.time,
            )?;
            channels.push(Rsm2TextureChannel { channel_type, keys });
        }
        animations.push(Rsm2TextureAnimation {
            texture_index,
            channels,
        });
    }
    Ok(animations)
}

fn validate_hierarchy(roots: &[String], nodes: &[Rsm2Node]) -> Result<(), Rsm2Error> {
    let mut node_indices = HashMap::with_capacity(nodes.len());
    for (index, node) in nodes.iter().enumerate() {
        if node_indices.insert(node.name.as_str(), index).is_some() {
            return Err(Rsm2Error::DuplicateNode {
                name: node.name.clone(),
            });
        }
    }

    let mut declared_roots = HashSet::with_capacity(roots.len());
    for root in roots {
        if !declared_roots.insert(root.as_str()) {
            return Err(Rsm2Error::DuplicateRoot { name: root.clone() });
        }
        let Some(&index) = node_indices.get(root.as_str()) else {
            return Err(Rsm2Error::MissingRoot { name: root.clone() });
        };
        if !nodes[index].parent_name.is_empty() {
            return Err(Rsm2Error::MissingRoot { name: root.clone() });
        }
    }

    for node in nodes {
        if node.parent_name.is_empty() {
            if !declared_roots.contains(node.name.as_str()) {
                return Err(Rsm2Error::OrphanNode {
                    name: node.name.clone(),
                });
            }
        } else if !node_indices.contains_key(node.parent_name.as_str()) {
            return Err(Rsm2Error::MissingParent {
                node: node.name.clone(),
                parent: node.parent_name.clone(),
            });
        }
    }

    for node in nodes {
        let mut path = HashSet::new();
        let mut current = node;
        while !current.parent_name.is_empty() {
            if !path.insert(current.name.as_str()) {
                return Err(Rsm2Error::NodeCycle {
                    name: current.name.clone(),
                });
            }
            current = &nodes[node_indices[current.parent_name.as_str()]];
        }
        if !declared_roots.contains(current.name.as_str()) {
            return Err(Rsm2Error::OrphanNode {
                name: node.name.clone(),
            });
        }
    }
    Ok(())
}

fn finite(value: f32, field: &str) -> Result<(), Rsm2Error> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(Rsm2Error::NonFinite {
            field: field.to_owned(),
        })
    }
}

struct Cursor<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len() - self.offset
    }

    fn bytes(&mut self, length: usize, field: &str) -> Result<&'a [u8], Rsm2Error> {
        let remaining = self.remaining();
        if remaining < length {
            return Err(Rsm2Error::Truncated {
                field: field.to_owned(),
                offset: self.offset,
                needed: length,
                remaining,
            });
        }
        let start = self.offset;
        self.offset += length;
        Ok(&self.data[start..self.offset])
    }

    fn array<const N: usize>(&mut self, field: &str) -> Result<[u8; N], Rsm2Error> {
        Ok(self.bytes(N, field)?.try_into().unwrap())
    }

    fn u8(&mut self, field: &str) -> Result<u8, Rsm2Error> {
        Ok(self.bytes(1, field)?[0])
    }

    fn i16(&mut self, field: &str) -> Result<i16, Rsm2Error> {
        Ok(i16::from_le_bytes(self.array(field)?))
    }

    fn i32(&mut self, field: &str) -> Result<i32, Rsm2Error> {
        Ok(i32::from_le_bytes(self.array(field)?))
    }

    fn f32(&mut self, field: &str) -> Result<f32, Rsm2Error> {
        Ok(f32::from_le_bytes(self.array(field)?))
    }

    fn f32_array<const N: usize>(&mut self, field: &str) -> Result<[f32; N], Rsm2Error> {
        let mut values = [0.0; N];
        for (index, value) in values.iter_mut().enumerate() {
            *value = self.f32(&format!("{field} component {index}"))?;
            finite(*value, field)?;
        }
        Ok(values)
    }

    fn count(&mut self, field: &str, maximum: i32) -> Result<usize, Rsm2Error> {
        let value = self.i32(field)?;
        if !(0..=maximum).contains(&value) {
            return Err(Rsm2Error::InvalidCount {
                field: field.to_owned(),
                value,
                maximum,
            });
        }
        Ok(value as usize)
    }

    fn dynamic_string(&mut self, field: &str, nonempty: bool) -> Result<String, Rsm2Error> {
        let length = self.i32(&format!("{field} length"))?;
        if !(0..=MAX_STRING_LENGTH).contains(&length) {
            return Err(Rsm2Error::InvalidStringLength {
                field: field.to_owned(),
                length,
            });
        }
        let bytes = self.bytes(length as usize, field)?;
        let (_, value) =
            parse_korean_string(bytes, bytes.len()).map_err(|_| Rsm2Error::InvalidString {
                field: field.to_owned(),
            })?;
        if nonempty && value.is_empty() {
            return Err(Rsm2Error::InvalidString {
                field: field.to_owned(),
            });
        }
        Ok(value)
    }

    fn index(&mut self, field: &str, length: usize) -> Result<usize, Rsm2Error> {
        let value = self.i32(field)?;
        if value < 0 || value as usize >= length {
            return Err(Rsm2Error::IndexOutOfRange {
                field: field.to_owned(),
                index: i64::from(value),
                length,
            });
        }
        Ok(value as usize)
    }

    fn i16_index(&mut self, field: &str, length: usize) -> Result<usize, Rsm2Error> {
        let value = self.i16(field)?;
        if value < 0 || value as usize >= length {
            return Err(Rsm2Error::IndexOutOfRange {
                field: field.to_owned(),
                index: i64::from(value),
                length,
            });
        }
        Ok(value as usize)
    }

    fn u16_indices(&mut self, field: &str, length: usize) -> Result<[usize; 3], Rsm2Error> {
        let mut values = [0; 3];
        for (index, value) in values.iter_mut().enumerate() {
            let raw = u16::from_le_bytes(self.array(&format!("{field} {index}"))?) as usize;
            if raw >= length {
                return Err(Rsm2Error::IndexOutOfRange {
                    field: format!("{field} {index}"),
                    index: raw as i64,
                    length,
                });
            }
            *value = raw;
        }
        Ok(values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FixtureOffsets {
        first_node_name_length: usize,
        first_texture_index: usize,
        first_vertex_index: usize,
        first_uv_index: usize,
        first_face_texture_index: usize,
        first_face_length: usize,
        first_scale_time: usize,
        first_scale_value: usize,
        first_channel_type: usize,
        first_channel_time: usize,
        volume_count: usize,
    }

    struct Writer {
        bytes: Vec<u8>,
        offsets: FixtureOffsets,
    }

    impl Writer {
        fn new() -> Self {
            Self {
                bytes: Vec::new(),
                offsets: FixtureOffsets::default(),
            }
        }

        fn u8(&mut self, value: u8) {
            self.bytes.push(value);
        }

        fn i16(&mut self, value: i16) {
            self.bytes.extend(value.to_le_bytes());
        }

        fn u16(&mut self, value: u16) {
            self.bytes.extend(value.to_le_bytes());
        }

        fn i32(&mut self, value: i32) {
            self.bytes.extend(value.to_le_bytes());
        }

        fn f32(&mut self, value: f32) {
            self.bytes.extend(value.to_le_bytes());
        }

        fn string(&mut self, value: &[u8]) {
            self.i32(value.len() as i32);
            self.bytes.extend(value);
        }

        fn vec3(&mut self, values: [f32; 3]) {
            for value in values {
                self.f32(value);
            }
        }

        fn node(&mut self, version: Rsm2Version, name: &[u8], parent: &[u8], rich: bool) {
            if self.offsets.first_node_name_length == 0 {
                self.offsets.first_node_name_length = self.bytes.len();
            }
            self.string(name);
            self.string(parent);
            self.i32(2);
            match version {
                Rsm2Version::V2_2 => {
                    if self.offsets.first_texture_index == 0 {
                        self.offsets.first_texture_index = self.bytes.len();
                    }
                    self.i32(0);
                    self.i32(1);
                }
                Rsm2Version::V2_3 => {
                    self.string(b"body.bmp");
                    self.string(b"detail.bmp");
                }
            }
            for value in [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0] {
                self.f32(value);
            }
            self.vec3([1.0, 2.0, 3.0]);
            self.i32(3);
            self.vec3([0.0, 0.0, 0.0]);
            self.vec3([1.0, 0.0, 0.0]);
            self.vec3([0.0, 1.0, 0.0]);
            self.i32(3);
            for coordinates in [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]] {
                self.f32(0.25);
                self.f32(coordinates[0]);
                self.f32(coordinates[1]);
            }
            self.i32(1);
            if self.offsets.first_face_length == 0 {
                self.offsets.first_face_length = self.bytes.len();
            }
            self.i32(if rich { 32 } else { 24 });
            if self.offsets.first_vertex_index == 0 {
                self.offsets.first_vertex_index = self.bytes.len();
            }
            for index in 0..3 {
                self.u16(index);
            }
            if self.offsets.first_uv_index == 0 {
                self.offsets.first_uv_index = self.bytes.len();
            }
            for index in 0..3 {
                self.u16(index);
            }
            if self.offsets.first_face_texture_index == 0 {
                self.offsets.first_face_texture_index = self.bytes.len();
            }
            self.i16(0);
            self.i16(7);
            self.i32(1);
            self.i32(11);
            if rich {
                self.i32(12);
                self.i32(13);
            }

            self.i32(2);
            if self.offsets.first_scale_time == 0 {
                self.offsets.first_scale_time = self.bytes.len();
            }
            for (time, scale) in [(3, [1.0, 1.0, 1.0]), (9, [2.0, 2.0, 2.0])] {
                self.i32(time);
                if self.offsets.first_scale_value == 0 {
                    self.offsets.first_scale_value = self.bytes.len();
                }
                self.vec3(scale);
                self.f32(0.5);
            }
            self.i32(2);
            for (time, quaternion) in [(4, [0.0, 0.0, 0.0, 1.0]), (10, [0.0, 0.0, 1.0, 0.0])] {
                self.i32(time);
                for value in quaternion {
                    self.f32(value);
                }
            }
            self.i32(2);
            for (time, position) in [(5, [1.0, 2.0, 3.0]), (11, [4.0, 5.0, 6.0])] {
                self.i32(time);
                self.vec3(position);
                self.f32(0.75);
            }

            if version == Rsm2Version::V2_3 {
                self.i32(1);
                self.i32(0);
                self.i32(5);
                for channel_type in 0..5 {
                    if self.offsets.first_channel_type == 0 {
                        self.offsets.first_channel_type = self.bytes.len();
                    }
                    self.i32(channel_type);
                    self.i32(2);
                    if self.offsets.first_channel_time == 0 {
                        self.offsets.first_channel_time = self.bytes.len();
                    }
                    self.i32(2);
                    self.f32(channel_type as f32 + 0.25);
                    self.i32(8);
                    self.f32(channel_type as f32 + 0.75);
                }
            }
        }
    }

    fn fixture(version: Rsm2Version) -> Writer {
        let mut writer = Writer::new();
        writer.bytes.extend(b"GRSM");
        writer.u8(2);
        writer.u8(if version == Rsm2Version::V2_2 { 2 } else { 3 });
        writer.i32(120);
        writer.i32(2);
        writer.u8(200);
        writer.f32(30.0);
        if version == Rsm2Version::V2_2 {
            writer.i32(2);
            writer.string(b"body.bmp");
            writer.string(b"detail.bmp");
        }
        writer.i32(2);
        writer.string(b"root");
        writer.string(b"other");
        writer.i32(3);
        writer.node(version, b"root", b"", true);
        writer.node(version, b"child", b"root", false);
        writer.node(version, b"other", b"", false);
        writer.offsets.volume_count = writer.bytes.len();
        writer.i32(0);
        writer
    }

    fn put_i32(bytes: &mut [u8], offset: usize, value: i32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn put_i16(bytes: &mut [u8], offset: usize, value: i16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn face_with_unknown_words(words: &[i32]) -> Vec<u8> {
        let fixture = fixture(Rsm2Version::V2_2);
        let mut bytes = fixture.bytes;
        put_i32(
            &mut bytes,
            fixture.offsets.first_face_length,
            32 + (words.len() * 4) as i32,
        );
        let insertion = fixture.offsets.first_face_length + 36;
        bytes.splice(
            insertion..insertion,
            words.iter().flat_map(|word| word.to_le_bytes()),
        );
        bytes
    }

    #[test]
    fn parses_strict_2_2_global_texture_layout() {
        let model = Rsm2::from_bytes(&fixture(Rsm2Version::V2_2).bytes).unwrap();
        assert_eq!(model.version, Rsm2Version::V2_2);
        assert_eq!(model.animation_length, 120);
        assert_eq!(model.frames_per_second, 30.0);
        assert_eq!(model.roots, ["root", "other"]);
        assert_eq!(model.global_textures, ["body.bmp", "detail.bmp"]);
        assert_eq!(
            model.nodes[0].textures,
            Rsm2NodeTextures::GlobalIndices(vec![0, 1])
        );
        assert_eq!(model.nodes[0].faces[0].smooth_groups, [11, 12, 13]);
        assert!(model.nodes[0].faces[0].unknown_words.is_empty());
        assert_eq!(model.nodes[0].scale_keys[0].time, 3);
        assert_eq!(model.nodes[0].rotation_keys[0].time, 4);
        assert_eq!(model.nodes[0].position_keys[0].time, 5);
        assert!(model.nodes[0].texture_animations.is_empty());
    }

    #[test]
    fn parses_strict_2_3_names_geometry_and_all_uv_channels() {
        let model = Rsm2::from_bytes(&fixture(Rsm2Version::V2_3).bytes).unwrap();
        assert_eq!(model.version, Rsm2Version::V2_3);
        assert!(model.global_textures.is_empty());
        assert_eq!(
            model.nodes[0].textures,
            Rsm2NodeTextures::Names(vec!["body.bmp".into(), "detail.bmp".into()])
        );
        assert_eq!(model.nodes[0].vertices.len(), 3);
        assert_eq!(model.nodes[0].texture_vertices[0].unknown, 0.25);
        assert_eq!(model.nodes[0].texture_animations[0].texture_index, 0);
        assert_eq!(
            model.nodes[0].texture_animations[0]
                .channels
                .iter()
                .map(|channel| channel.channel_type)
                .collect::<Vec<_>>(),
            [
                Rsm2TextureChannelType::TranslateU,
                Rsm2TextureChannelType::TranslateV,
                Rsm2TextureChannelType::ScaleU,
                Rsm2TextureChannelType::ScaleV,
                Rsm2TextureChannelType::Rotate,
            ]
        );
        assert_eq!(
            model.nodes[0].texture_animations[0].channels[0].keys[0].time,
            2
        );
    }

    #[test]
    fn parses_face_unknown_words_after_three_smooth_groups() {
        for words in [&[21][..], &[21, 22], &[21, 22, 23]] {
            let face = &Rsm2::from_bytes(&face_with_unknown_words(words))
                .unwrap()
                .nodes[0]
                .faces[0];
            assert_eq!(face.smooth_groups, [11, 12, 13]);
            assert_eq!(face.unknown_words, words);
        }
    }

    #[test]
    fn rejects_bad_magic_versions_truncation_and_trailing_bytes() {
        let mut bad_magic = fixture(Rsm2Version::V2_2).bytes;
        bad_magic[0] = b'X';
        assert!(matches!(
            Rsm2::from_bytes(&bad_magic),
            Err(Rsm2Error::InvalidMagic(_))
        ));

        let mut bad_version = fixture(Rsm2Version::V2_2).bytes;
        bad_version[5] = 4;
        assert!(matches!(
            Rsm2::from_bytes(&bad_version),
            Err(Rsm2Error::UnsupportedVersion { .. })
        ));
        assert!(matches!(
            Rsm2::from_bytes(b"GRSM\x02"),
            Err(Rsm2Error::Truncated { .. })
        ));

        let mut trailing = fixture(Rsm2Version::V2_2).bytes;
        trailing.push(0);
        assert_eq!(
            Rsm2::from_bytes(&trailing),
            Err(Rsm2Error::TrailingBytes { count: 1 })
        );
    }

    #[test]
    fn rejects_negative_oversized_counts_and_invalid_strings() {
        let mut negative = fixture(Rsm2Version::V2_2).bytes;
        put_i32(&mut negative, 19, -1);
        assert!(matches!(
            Rsm2::from_bytes(&negative),
            Err(Rsm2Error::InvalidCount { value: -1, .. })
        ));

        let mut oversized = fixture(Rsm2Version::V2_2).bytes;
        put_i32(&mut oversized, 19, 101);
        assert!(matches!(
            Rsm2::from_bytes(&oversized),
            Err(Rsm2Error::InvalidCount { value: 101, .. })
        ));

        let invalid_fixture = fixture(Rsm2Version::V2_2);
        let mut invalid = invalid_fixture.bytes;
        put_i32(
            &mut invalid,
            invalid_fixture.offsets.first_node_name_length,
            1025,
        );
        assert!(matches!(
            Rsm2::from_bytes(&invalid),
            Err(Rsm2Error::InvalidStringLength { length: 1025, .. })
        ));

        let empty_fixture = fixture(Rsm2Version::V2_2);
        let mut empty = empty_fixture.bytes;
        put_i32(&mut empty, empty_fixture.offsets.first_node_name_length, 0);
        assert!(matches!(
            Rsm2::from_bytes(&empty),
            Err(Rsm2Error::InvalidString { .. })
        ));
    }

    #[test]
    fn rejects_bad_face_lengths_and_all_out_of_range_indices() {
        let fixture = fixture(Rsm2Version::V2_2);
        for bad_length in [20, 25] {
            let mut bytes = fixture.bytes.clone();
            put_i32(&mut bytes, fixture.offsets.first_face_length, bad_length);
            assert!(matches!(
                Rsm2::from_bytes(&bytes),
                Err(Rsm2Error::InvalidFaceLength { .. })
            ));
        }

        for (offset, wide) in [
            (fixture.offsets.first_texture_index, true),
            (fixture.offsets.first_vertex_index, false),
            (fixture.offsets.first_uv_index, false),
            (fixture.offsets.first_face_texture_index, false),
        ] {
            let mut bytes = fixture.bytes.clone();
            if wide {
                put_i32(&mut bytes, offset, -1);
            } else {
                put_i16(&mut bytes, offset, -1);
            }
            assert!(matches!(
                Rsm2::from_bytes(&bytes),
                Err(Rsm2Error::IndexOutOfRange { .. })
            ));
        }
    }

    #[test]
    fn rejects_non_finite_non_increasing_and_unknown_uv_keys() {
        let fixture = fixture(Rsm2Version::V2_3);
        let mut non_finite = fixture.bytes.clone();
        non_finite[fixture.offsets.first_scale_value..fixture.offsets.first_scale_value + 4]
            .copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(matches!(
            Rsm2::from_bytes(&non_finite),
            Err(Rsm2Error::NonFinite { .. })
        ));

        let mut non_increasing = fixture.bytes.clone();
        put_i32(
            &mut non_increasing,
            fixture.offsets.first_scale_time + 20,
            3,
        );
        assert!(matches!(
            Rsm2::from_bytes(&non_increasing),
            Err(Rsm2Error::NonIncreasingKey { .. })
        ));

        let mut bad_channel = fixture.bytes.clone();
        put_i32(&mut bad_channel, fixture.offsets.first_channel_type, 5);
        assert!(matches!(
            Rsm2::from_bytes(&bad_channel),
            Err(Rsm2Error::UnknownTextureChannel { value: 5, .. })
        ));

        let mut bad_channel_time = fixture.bytes;
        put_i32(
            &mut bad_channel_time,
            fixture.offsets.first_channel_time + 8,
            2,
        );
        assert!(matches!(
            Rsm2::from_bytes(&bad_channel_time),
            Err(Rsm2Error::NonIncreasingKey { .. })
        ));
    }

    #[test]
    fn rejects_duplicate_missing_orphaned_and_cyclic_nodes() {
        let duplicate = hierarchy_fixture(&[b"root", b"root"], &[b"", b""], &[b"root"]);
        assert!(matches!(
            Rsm2::from_bytes(&duplicate),
            Err(Rsm2Error::DuplicateNode { .. })
        ));

        let missing = hierarchy_fixture(&[b"root"], &[b""], &[b"missing"]);
        assert!(matches!(
            Rsm2::from_bytes(&missing),
            Err(Rsm2Error::MissingRoot { .. })
        ));

        let missing_parent =
            hierarchy_fixture(&[b"root", b"child"], &[b"", b"missing"], &[b"root"]);
        assert!(matches!(
            Rsm2::from_bytes(&missing_parent),
            Err(Rsm2Error::MissingParent { .. })
        ));

        let orphan = hierarchy_fixture(&[b"root", b"other"], &[b"", b""], &[b"root"]);
        assert!(matches!(
            Rsm2::from_bytes(&orphan),
            Err(Rsm2Error::OrphanNode { .. })
        ));

        let cycle = hierarchy_fixture(
            &[b"root", b"one", b"two"],
            &[b"", b"two", b"one"],
            &[b"root"],
        );
        assert!(matches!(
            Rsm2::from_bytes(&cycle),
            Err(Rsm2Error::NodeCycle { .. })
        ));
    }

    fn hierarchy_fixture(names: &[&[u8]], parents: &[&[u8]], roots: &[&[u8]]) -> Vec<u8> {
        let mut writer = Writer::new();
        writer.bytes.extend(b"GRSM");
        writer.u8(2);
        writer.u8(2);
        writer.i32(1);
        writer.i32(0);
        writer.u8(255);
        writer.f32(1.0);
        writer.i32(1);
        writer.string(b"texture.bmp");
        writer.i32(roots.len() as i32);
        for root in roots {
            writer.string(root);
        }
        writer.i32(names.len() as i32);
        for (name, parent) in names.iter().zip(parents) {
            writer.string(name);
            writer.string(parent);
            writer.i32(1);
            writer.i32(0);
            for value in [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0] {
                writer.f32(value);
            }
            writer.vec3([0.0; 3]);
            writer.i32(0);
            writer.i32(0);
            writer.i32(0);
            writer.i32(0);
            writer.i32(0);
            writer.i32(0);
        }
        writer.i32(0);
        writer.bytes
    }

    #[test]
    fn accepts_absent_and_explicit_zero_volume_trailers_and_rejects_others() {
        let fixture = fixture(Rsm2Version::V2_2);
        assert!(Rsm2::from_bytes(&fixture.bytes).is_ok());

        let mut absent = fixture.bytes.clone();
        absent.truncate(fixture.offsets.volume_count);
        assert!(Rsm2::from_bytes(&absent).is_ok());

        let mut nonzero = fixture.bytes;
        put_i32(&mut nonzero, fixture.offsets.volume_count, 1);
        assert_eq!(
            Rsm2::from_bytes(&nonzero),
            Err(Rsm2Error::UnsupportedVolumes { count: 1 })
        );

        for trailing_byte_count in 1..=3 {
            let mut short = absent.clone();
            short.extend(vec![0; trailing_byte_count]);
            assert_eq!(
                Rsm2::from_bytes(&short),
                Err(Rsm2Error::Truncated {
                    field: "volume count".to_owned(),
                    offset: fixture.offsets.volume_count,
                    needed: 4,
                    remaining: trailing_byte_count,
                })
            );
        }
    }
}
