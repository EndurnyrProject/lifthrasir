use crate::string_utils::parse_korean_string;
use nalgebra::{Matrix4, Vector3, Vector4};
use nom::{
    IResult,
    bytes::complete::{tag, take},
    number::complete::{le_f32, le_i32, le_u8, le_u16},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RsmError {
    #[error("Failed to parse RSM file: {0}")]
    ParseError(String),
    #[error(
        "RSM version {version} left {actual} unconsumed byte(s), expected {expected}; \
         the layout is out of sync with the file"
    )]
    TrailingBytes {
        version: String,
        actual: usize,
        expected: usize,
    },
}

/// Encoded `major << 8 | minor`, mirroring BrowEdit's `0x0104`-style constants.
///
/// The version is compared as an integer rather than through the `f32` field:
/// `1.6` is not exactly representable, so float comparisons around the version
/// gates are not trustworthy.
type Version = u16;

const V1_2: Version = 0x0102;
const V1_3: Version = 0x0103;
const V1_4: Version = 0x0104;
const V1_5: Version = 0x0105;
const V1_6: Version = 0x0106;

pub type RsmFile = Rsm;

#[derive(Debug, Clone)]
pub struct BoundingBox {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub center: [f32; 3],
    pub range: [f32; 3],
}

impl Default for BoundingBox {
    fn default() -> Self {
        Self::new()
    }
}

impl BoundingBox {
    pub fn new() -> Self {
        Self {
            min: [f32::MAX, f32::MAX, f32::MAX],
            max: [f32::MIN, f32::MIN, f32::MIN],
            center: [0.0, 0.0, 0.0],
            range: [0.0, 0.0, 0.0],
        }
    }

    pub fn update(&mut self, point: &[f32; 3]) {
        for ((&p, min), max) in point.iter().zip(&mut self.min).zip(&mut self.max) {
            *min = (*min).min(p);
            *max = (*max).max(p);
        }
    }

    pub fn finalize(&mut self) {
        for i in 0..3 {
            self.range[i] = (self.max[i] - self.min[i]) / 2.0;
            self.center[i] = self.min[i] + self.range[i];
        }
    }
}

#[derive(Debug, Clone)]
pub struct Rsm {
    pub version: f32,
    /// `major << 8 | minor`, the form the version gates are compared against.
    pub raw_version: Version,
    pub anim_len: i32,
    pub shade_type: ShadingType,
    pub alpha: f32,
    pub textures: Vec<String>,
    pub main_node_name: String,
    pub nodes: Vec<Node>,
    /// Model-wide scale keyframes, present only below version 1.6.
    ///
    /// These are *scale* frames, not position frames: `int frame; vec3 scale;
    /// float data` (20 bytes). Nothing consumes them yet, but they must be read
    /// to reach the volume boxes.
    pub scale_keyframes: Vec<ScaleKeyframe>,
    pub volume_boxes: Vec<VolumeBox>,
    pub bounding_box: Option<BoundingBox>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShadingType {
    None = 0,
    Flat = 1,
    Smooth = 2,
}

impl From<i32> for ShadingType {
    fn from(value: i32) -> Self {
        match value {
            1 => ShadingType::Flat,
            2 => ShadingType::Smooth,
            _ => ShadingType::None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Node {
    pub name: String,
    pub parent_name: String,
    pub texture_ids: Vec<i32>,
    pub mat3: [f32; 9],
    pub offset: [f32; 3],
    pub pos: [f32; 3],
    pub rot_angle: f32,
    pub rot_axis: [f32; 3],
    pub scale: [f32; 3],
    pub vertices: Vec<[f32; 3]>,
    pub texture_vertices: Vec<TextureVertex>,
    pub faces: Vec<Face>,
    /// RSM1 nodes carry rotation keyframes only.
    ///
    /// Per-node *position* keyframes are an RSM2 (>= 2.2) feature and must not
    /// be read here; doing so desynchronises every following field.
    pub rot_keyframes: Vec<RotKeyframe>,
}

#[derive(Debug, Clone)]
pub struct TextureVertex {
    pub color: Option<[u8; 4]>,
    pub u: f32,
    pub v: f32,
}

#[derive(Debug, Clone)]
pub struct Face {
    pub vertex_ids: [u16; 3],
    pub texture_vertex_ids: [u16; 3],
    pub tex_id: u16,
    pub padding: u16,
    pub two_side: i32,
    pub smooth_group: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScaleKeyframe {
    pub frame: i32,
    pub scale: [f32; 3],
    pub data: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RotKeyframe {
    pub frame: i32,
    pub q: [f32; 4],
}

#[derive(Debug, Clone)]
pub struct VolumeBox {
    pub size: [f32; 3],
    pub pos: [f32; 3],
    pub rot: [f32; 3],
    pub flag: i32,
}

impl Rsm {
    pub fn from_bytes(data: &[u8]) -> Result<Self, RsmError> {
        let (remaining, mut rsm) =
            parse_rsm(data).map_err(|e| RsmError::ParseError(format!("{e:?}")))?;

        // A layout that is out of sync usually still "parses" - it just stops in
        // the wrong place. Requiring the parse to land on the end of the file is
        // what turns a silent mis-parse into a loud failure.
        let expected = expected_trailing_bytes(rsm.raw_version);
        if remaining.len() != expected {
            return Err(RsmError::TrailingBytes {
                version: format!("{:.1}", rsm.version),
                actual: remaining.len(),
                expected,
            });
        }

        rsm.calculate_bounding_box();
        Ok(rsm)
    }

    pub fn calculate_bounding_box(&mut self) {
        let mut bbox = BoundingBox::new();

        // Find the main node
        let main_node_idx = self
            .nodes
            .iter()
            .position(|n| n.name == self.main_node_name)
            .unwrap_or(0);

        // Build parent-child relationships
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); self.nodes.len()];
        for (idx, node) in self.nodes.iter().enumerate() {
            if !node.parent_name.is_empty()
                && node.name != node.parent_name
                && let Some(parent_idx) = self.nodes.iter().position(|n| n.name == node.parent_name)
            {
                children[parent_idx].push(idx);
            }
        }

        // Start with identity matrix
        let identity = Matrix4::<f32>::identity();

        // Calculate bounding box starting from main node
        self.calculate_node_bbox(main_node_idx, &identity, &children, &mut bbox);

        bbox.finalize();
        self.bounding_box = Some(bbox);
    }

    fn mat3_to_mat4(mat3: &[f32; 9]) -> Matrix4<f32> {
        // Convert 3x3 matrix to 4x4 format
        // RoBrowser stores mat3 in column-major order
        Matrix4::new(
            mat3[0], mat3[3], mat3[6], 0.0, mat3[1], mat3[4], mat3[7], 0.0, mat3[2], mat3[5],
            mat3[8], 0.0, 0.0, 0.0, 0.0, 1.0,
        )
    }

    fn calculate_node_bbox(
        &self,
        node_idx: usize,
        parent_matrix: &Matrix4<f32>,
        children: &[Vec<usize>],
        bbox: &mut BoundingBox,
    ) {
        let node = &self.nodes[node_idx];
        let is_only = self.nodes.len() == 1;

        // Build transformation matrix for this node
        let mut transform = *parent_matrix;

        // Apply position
        let translation =
            Matrix4::new_translation(&Vector3::new(node.pos[0], node.pos[1], node.pos[2]));
        transform *= translation;

        // Apply rotation (if no keyframes, use static rotation)
        if node.rot_keyframes.is_empty() && node.rot_angle != 0.0 {
            let axis = Vector3::new(node.rot_axis[0], node.rot_axis[1], node.rot_axis[2]);
            if axis.magnitude() > 0.0 {
                let unit_axis = nalgebra::Unit::new_normalize(axis);
                let rotation = Matrix4::from_axis_angle(&unit_axis, node.rot_angle);
                transform *= rotation;
            }
        }

        // Apply scale
        let scale = Matrix4::new_nonuniform_scaling(&Vector3::new(
            node.scale[0],
            node.scale[1],
            node.scale[2],
        ));
        transform *= scale;

        // Create local matrix for vertices
        let mut local_transform = transform;

        // Apply offset (unless it's the only node)
        if !is_only {
            let offset = Matrix4::new_translation(&Vector3::new(
                node.offset[0],
                node.offset[1],
                node.offset[2],
            ));
            local_transform *= offset;
        }

        // Apply mat3 transformation
        let mat3_transform = Self::mat3_to_mat4(&node.mat3);
        local_transform *= mat3_transform;

        // Transform vertices and update bounding box
        for vertex in &node.vertices {
            let v = Vector4::new(vertex[0], vertex[1], vertex[2], 1.0);
            let transformed = local_transform * v;
            bbox.update(&[transformed.x, transformed.y, transformed.z]);
        }

        // Process children recursively with accumulated transform
        for &child_idx in &children[node_idx] {
            self.calculate_node_bbox(child_idx, &transform, children, bbox);
        }
    }
}

/// Read a non-negative element count.
///
/// The counts are stored as signed 32-bit integers. `for _ in 0..count` is an
/// *empty* range when `count` is negative, so a desynchronised parse that lands
/// on garbage silently yields an empty list and "succeeds" instead of failing.
/// Rejecting negative counts turns that class of corruption into a hard error.
fn parse_count(input: &[u8]) -> IResult<&[u8], usize> {
    let (input, count) = le_i32(input)?;
    let count = usize::try_from(count).map_err(|_| {
        nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Verify))
    })?;
    Ok((input, count))
}

fn parse_header(input: &[u8]) -> IResult<&[u8], (Version, i32, i32, f32)> {
    let (input, _) = tag(&b"GRSM"[..])(input)?;
    let (input, major) = le_u8(input)?;
    let (input, minor) = le_u8(input)?;
    let version = ((major as Version) << 8) | minor as Version;

    let (input, anim_len) = le_i32(input)?;
    let (input, shade_type) = le_i32(input)?;

    let (input, alpha) = if version >= V1_4 {
        let (input, a) = le_u8(input)?;
        (input, a as f32 / 255.0)
    } else {
        (input, 1.0)
    };

    let (input, _) = take(16usize)(input)?;

    Ok((input, (version, anim_len, shade_type, alpha)))
}

fn parse_textures(input: &[u8]) -> IResult<&[u8], Vec<String>> {
    let (input, tex_count) = parse_count(input)?;
    let mut remaining = input;
    let mut textures = Vec::with_capacity(tex_count);

    for _ in 0..tex_count {
        let (new_remaining, texture) = parse_korean_string(remaining, 40)?;
        textures.push(texture);
        remaining = new_remaining;
    }

    Ok((remaining, textures))
}

fn parse_texture_vertex(input: &[u8], version: Version) -> IResult<&[u8], TextureVertex> {
    let (input, color) = if version >= V1_2 {
        let (input, r) = le_u8(input)?;
        let (input, g) = le_u8(input)?;
        let (input, b) = le_u8(input)?;
        let (input, a) = le_u8(input)?;
        (input, Some([r, g, b, a]))
    } else {
        (input, None)
    };

    let (input, u) = le_f32(input)?;
    let (input, v) = le_f32(input)?;

    let u = u * 0.98 + 0.01;
    let v = v * 0.98 + 0.01;

    Ok((input, TextureVertex { color, u, v }))
}

fn parse_face(input: &[u8], version: Version) -> IResult<&[u8], Face> {
    let (input, v0) = le_u16(input)?;
    let (input, v1) = le_u16(input)?;
    let (input, v2) = le_u16(input)?;
    let (input, t0) = le_u16(input)?;
    let (input, t1) = le_u16(input)?;
    let (input, t2) = le_u16(input)?;
    let (input, tex_id) = le_u16(input)?;
    let (input, padding) = le_u16(input)?;
    let (input, two_side) = le_i32(input)?;

    let (input, smooth_group) = if version >= V1_2 {
        le_i32(input)?
    } else {
        (input, 0)
    };

    Ok((
        input,
        Face {
            vertex_ids: [v0, v1, v2],
            texture_vertex_ids: [t0, t1, t2],
            tex_id,
            padding,
            two_side,
            smooth_group,
        },
    ))
}

fn parse_scale_keyframe(input: &[u8]) -> IResult<&[u8], ScaleKeyframe> {
    let (input, frame) = le_i32(input)?;
    let (input, scale) = parse_float_array::<3>(input)?;
    let (input, data) = le_f32(input)?;

    Ok((input, ScaleKeyframe { frame, scale, data }))
}

fn parse_rot_keyframe(input: &[u8]) -> IResult<&[u8], RotKeyframe> {
    let (input, frame) = le_i32(input)?;
    let (input, q0) = le_f32(input)?;
    let (input, q1) = le_f32(input)?;
    let (input, q2) = le_f32(input)?;
    let (input, q3) = le_f32(input)?;

    Ok((
        input,
        RotKeyframe {
            frame,
            q: [q0, q1, q2, q3],
        },
    ))
}

fn parse_float_array<const N: usize>(input: &[u8]) -> IResult<&[u8], [f32; N]> {
    let mut array = [0.0; N];
    let mut remaining = input;

    for item in &mut array {
        let (new_remaining, value) = le_f32(remaining)?;
        *item = value;
        remaining = new_remaining;
    }

    Ok((remaining, array))
}

fn parse_node(input: &[u8], version: Version, _is_only: bool) -> IResult<&[u8], Node> {
    let (input, name) = parse_korean_string(input, 40)?;
    let (input, parent_name) = parse_korean_string(input, 40)?;

    let (input, tex_count) = parse_count(input)?;
    let mut texture_ids = Vec::with_capacity(tex_count);
    let mut remaining = input;

    for _ in 0..tex_count {
        let (new_remaining, id) = le_i32(remaining)?;
        texture_ids.push(id);
        remaining = new_remaining;
    }

    let (remaining, mat3) = parse_float_array::<9>(remaining)?;

    let (remaining, offset) = parse_float_array::<3>(remaining)?;
    let (remaining, pos) = parse_float_array::<3>(remaining)?;
    let (remaining, rot_angle) = le_f32(remaining)?;
    let (remaining, rot_axis) = parse_float_array::<3>(remaining)?;
    let (remaining, scale) = parse_float_array::<3>(remaining)?;
    let (remaining, vert_count) = parse_count(remaining)?;

    let mut vertices = Vec::with_capacity(vert_count);
    let mut rem = remaining;

    for _ in 0..vert_count {
        let (new_rem, vertex) = parse_float_array::<3>(rem)?;
        vertices.push(vertex);
        rem = new_rem;
    }

    let (rem, tvert_count) = parse_count(rem)?;
    let mut texture_vertices = Vec::with_capacity(tvert_count);
    let mut remaining = rem;

    for _ in 0..tvert_count {
        let (new_remaining, tv) = parse_texture_vertex(remaining, version)?;
        texture_vertices.push(tv);
        remaining = new_remaining;
    }

    let (remaining, face_count) = parse_count(remaining)?;
    let mut faces = Vec::with_capacity(face_count);
    let mut rem = remaining;

    for _ in 0..face_count {
        let (new_rem, face) = parse_face(rem, version)?;
        faces.push(face);
        rem = new_rem;
    }

    // Rotation keyframes are present in every RSM1 node, at every version.
    //
    // There is no per-node *position* keyframe block here: that field was added
    // in RSM2 (>= 2.2) and is parsed by `rsm2.rs`. Reading one at 1.5 shifts
    // every subsequent field and silently corrupts the rest of the file.
    let (rem, rot_count) = parse_count(rem)?;
    let mut rot_keyframes = Vec::with_capacity(rot_count);
    let mut remaining = rem;

    for _ in 0..rot_count {
        let (new_remaining, kf) = parse_rot_keyframe(remaining)?;
        rot_keyframes.push(kf);
        remaining = new_remaining;
    }

    debug_assert!(
        rot_keyframes.windows(2).all(|w| w[0].frame <= w[1].frame),
        "Rotation keyframes must be sorted by frame number"
    );

    let rem = remaining;

    Ok((
        rem,
        Node {
            name,
            parent_name,
            texture_ids,
            mat3,
            offset,
            pos,
            rot_angle,
            rot_axis,
            scale,
            vertices,
            texture_vertices,
            faces,
            rot_keyframes,
        },
    ))
}

fn parse_volume_box(input: &[u8], version: Version) -> IResult<&[u8], VolumeBox> {
    let (input, size) = parse_float_array::<3>(input)?;
    let (input, pos) = parse_float_array::<3>(input)?;
    let (input, rot) = parse_float_array::<3>(input)?;

    let (input, flag) = if version >= V1_3 {
        le_i32(input)?
    } else {
        (input, 0)
    };

    Ok((
        input,
        VolumeBox {
            size,
            pos,
            rot,
            flag,
        },
    ))
}

pub fn parse_rsm(input: &[u8]) -> IResult<&[u8], Rsm> {
    let (input, (version, anim_len, shade_type, alpha)) = parse_header(input)?;
    let (input, textures) = parse_textures(input)?;

    let (input, main_node_name) = parse_korean_string(input, 40)?;
    let (input, node_count) = parse_count(input)?;

    let is_only = node_count == 1;
    let mut nodes = Vec::with_capacity(node_count);
    let mut remaining = input;

    for _ in 0..node_count {
        let (new_remaining, node) = parse_node(remaining, version, is_only)?;
        nodes.push(node);
        remaining = new_remaining;
    }

    // Model-wide *scale* keyframes, below version 1.6.
    //
    // Each entry is `int frame; vec3 scale; float data` (20 bytes). This block
    // used to be read as 16-byte position keyframes, which overran the volume
    // box section and ran the parser off the end of the file.
    let (remaining, scale_keyframes) = if version < V1_6 {
        let (rem, kf_count) = parse_count(remaining)?;
        let mut keyframes = Vec::with_capacity(kf_count);
        let mut remaining = rem;

        for _ in 0..kf_count {
            let (new_remaining, kf) = parse_scale_keyframe(remaining)?;
            keyframes.push(kf);
            remaining = new_remaining;
        }

        (remaining, keyframes)
    } else {
        (remaining, Vec::new())
    };

    // Parse volume boxes
    let (remaining, vol_count) = parse_count(remaining)?;
    let mut volume_boxes = Vec::with_capacity(vol_count);
    let mut rem = remaining;

    for _ in 0..vol_count {
        let (new_rem, vb) = parse_volume_box(rem, version)?;
        volume_boxes.push(vb);
        rem = new_rem;
    }

    Ok((
        rem,
        Rsm {
            version: version_to_f32(version),
            raw_version: version,
            anim_len,
            shade_type: ShadingType::from(shade_type),
            alpha,
            textures,
            main_node_name,
            nodes,
            scale_keyframes,
            volume_boxes,
            bounding_box: None,
        },
    ))
}

fn version_to_f32(version: Version) -> f32 {
    (version >> 8) as f32 + (version & 0xff) as f32 / 10.0
}

/// Bytes the format legitimately leaves behind after the volume boxes.
///
/// Version 1.5 files carry four trailing bytes that no known implementation
/// reads (BrowEdit stops before them too). Every other version ends exactly on
/// the last volume box.
fn expected_trailing_bytes(version: Version) -> usize {
    if version >= V1_5 { 4 } else { 0 }
}
