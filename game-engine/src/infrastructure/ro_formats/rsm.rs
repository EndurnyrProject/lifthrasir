use crate::utils::string_utils::parse_korean_string;
use nalgebra::{Matrix4, Vector3, Vector4};
use nom::{
    bytes::complete::{tag, take},
    number::complete::{le_f32, le_i32, le_u16, le_u8},
    IResult,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RsmError {
    #[error("Failed to parse RSM file: {0}")]
    ParseError(String),
}

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
        for i in 0..3 {
            self.min[i] = self.min[i].min(point[i]);
            self.max[i] = self.max[i].max(point[i]);
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
    pub anim_len: i32,
    pub shade_type: ShadingType,
    pub alpha: f32,
    pub textures: Vec<String>,
    pub main_node_name: String,
    pub nodes: Vec<Node>,
    pub pos_keyframes: Vec<PosKeyframe>,
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
    pub pos_keyframes: Vec<PosKeyframe>,
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

#[derive(Debug, Clone)]
pub struct PosKeyframe {
    pub frame: i32,
    pub px: f32,
    pub py: f32,
    pub pz: f32,
}

#[derive(Debug, Clone)]
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
        match parse_rsm(data) {
            Ok((_, mut rsm)) => {
                rsm.calculate_bounding_box();
                Ok(rsm)
            }
            Err(e) => Err(RsmError::ParseError(format!("{e:?}"))),
        }
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
            if !node.parent_name.is_empty() && node.name != node.parent_name {
                if let Some(parent_idx) = self.nodes.iter().position(|n| n.name == node.parent_name)
                {
                    children[parent_idx].push(idx);
                }
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

fn parse_header(input: &[u8]) -> IResult<&[u8], (f32, i32, i32, f32)> {
    let (input, _) = tag(&b"GRSM"[..])(input)?;
    let (input, major) = le_u8(input)?;
    let (input, minor) = le_u8(input)?;
    let version = major as f32 + minor as f32 / 10.0;

    let (input, anim_len) = le_i32(input)?;
    let (input, shade_type) = le_i32(input)?;

    let (input, alpha) = if version >= 1.4 {
        let (input, a) = le_u8(input)?;
        (input, a as f32 / 255.0)
    } else {
        (input, 1.0)
    };

    let (input, _) = take(16usize)(input)?;

    Ok((input, (version, anim_len, shade_type, alpha)))
}

fn parse_textures(input: &[u8]) -> IResult<&[u8], Vec<String>> {
    let (input, tex_count) = le_i32(input)?;
    let mut remaining = input;
    let mut textures = Vec::new();

    for _ in 0..tex_count {
        let (new_remaining, texture) = parse_korean_string(remaining, 40)?;
        textures.push(texture);
        remaining = new_remaining;
    }

    Ok((remaining, textures))
}

fn parse_texture_vertex(input: &[u8], version: f32) -> IResult<&[u8], TextureVertex> {
    let (input, color) = if version >= 1.2 {
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

fn parse_face(input: &[u8], version: f32) -> IResult<&[u8], Face> {
    let (input, v0) = le_u16(input)?;
    let (input, v1) = le_u16(input)?;
    let (input, v2) = le_u16(input)?;
    let (input, t0) = le_u16(input)?;
    let (input, t1) = le_u16(input)?;
    let (input, t2) = le_u16(input)?;
    let (input, tex_id) = le_u16(input)?;
    let (input, padding) = le_u16(input)?;
    let (input, two_side) = le_i32(input)?;

    let (input, smooth_group) = if version >= 1.2 {
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

fn parse_pos_keyframe(input: &[u8]) -> IResult<&[u8], PosKeyframe> {
    let (input, frame) = le_i32(input)?;
    let (input, px) = le_f32(input)?;
    let (input, py) = le_f32(input)?;
    let (input, pz) = le_f32(input)?;

    Ok((input, PosKeyframe { frame, px, py, pz }))
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

    for i in 0..N {
        let (new_remaining, value) = le_f32(remaining)?;
        array[i] = value;
        remaining = new_remaining;
    }

    Ok((remaining, array))
}

fn parse_node(input: &[u8], version: f32, _is_only: bool) -> IResult<&[u8], Node> {
    let (input, name) = parse_korean_string(input, 40)?;
    let (input, parent_name) = parse_korean_string(input, 40)?;

    let (input, tex_count) = le_i32(input)?;
    let mut texture_ids = Vec::new();
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
    let (remaining, vert_count) = le_i32(remaining)?;

    let mut vertices = Vec::new();
    let mut rem = remaining;

    for _ in 0..vert_count {
        let (new_rem, vertex) = parse_float_array::<3>(rem)?;
        vertices.push(vertex);
        rem = new_rem;
    }

    let (rem, tvert_count) = le_i32(rem)?;
    let mut texture_vertices = Vec::new();
    let mut remaining = rem;

    for _ in 0..tvert_count {
        let (new_remaining, tv) = parse_texture_vertex(remaining, version)?;
        texture_vertices.push(tv);
        remaining = new_remaining;
    }

    let (remaining, face_count) = le_i32(remaining)?;
    let mut faces = Vec::new();
    let mut rem = remaining;

    for _ in 0..face_count {
        let (new_rem, face) = parse_face(rem, version)?;
        faces.push(face);
        rem = new_rem;
    }

    let (rem, pos_keyframes) = if version >= 1.5 {
        let (rem, kf_count) = le_i32(rem)?;
        let mut keyframes = Vec::new();
        let mut remaining = rem;

        for _ in 0..kf_count {
            let (new_remaining, kf) = parse_pos_keyframe(remaining)?;
            keyframes.push(kf);
            remaining = new_remaining;
        }

        (remaining, keyframes)
    } else {
        (rem, Vec::new())
    };

    // For version 1.5, rotation keyframes are NOT present in nodes
    // They're stored separately at the RSM level
    let (rem, rot_keyframes) = if version < 1.5 {
        // For older versions, rotation keyframes are in each node
        let (rem2, rot_count) = le_i32(rem)?;
        let mut rot_keyframes = Vec::new();
        let mut remaining = rem2;

        for _ in 0..rot_count {
            let (new_remaining, kf) = parse_rot_keyframe(remaining)?;
            rot_keyframes.push(kf);
            remaining = new_remaining;
        }
        (remaining, rot_keyframes)
    } else {
        // For version 1.5+, rotation keyframes are not in individual nodes
        (rem, Vec::new())
    };

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
            pos_keyframes,
            rot_keyframes,
        },
    ))
}

fn parse_volume_box(input: &[u8], version: f32) -> IResult<&[u8], VolumeBox> {
    let (input, size) = parse_float_array::<3>(input)?;
    let (input, pos) = parse_float_array::<3>(input)?;
    let (input, rot) = parse_float_array::<3>(input)?;

    let (input, flag) = if version >= 1.3 {
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
    let (input, node_count) = le_i32(input)?;

    let is_only = node_count == 1;
    let mut nodes = Vec::new();
    let mut remaining = input;

    for _ in 0..node_count {
        let (new_remaining, node) = parse_node(remaining, version, is_only)?;
        nodes.push(node);
        remaining = new_remaining;
    }

    // Parse global position keyframes (version < 1.5)
    let (remaining, pos_keyframes) = if version < 1.5 {
        let (rem, kf_count) = le_i32(remaining)?;
        let mut keyframes = Vec::new();
        let mut remaining = rem;

        for _ in 0..kf_count {
            let (new_remaining, kf) = parse_pos_keyframe(remaining)?;
            keyframes.push(kf);
            remaining = new_remaining;
        }

        (remaining, keyframes)
    } else {
        (remaining, Vec::new())
    };

    // Parse volume boxes
    let (remaining, vol_count) = le_i32(remaining)?;
    let mut volume_boxes = Vec::new();
    let mut rem = remaining;

    for _ in 0..vol_count {
        let (new_rem, vb) = parse_volume_box(rem, version)?;
        volume_boxes.push(vb);
        rem = new_rem;
    }

    Ok((
        rem,
        Rsm {
            version,
            anim_len,
            shade_type: ShadingType::from(shade_type),
            alpha,
            textures,
            main_node_name,
            nodes,
            pos_keyframes,
            volume_boxes,
            bounding_box: None,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_parse_cherry_tree_01() {
        let data = fs::read("assets/cherrytree_s_01.rsm").expect("Failed to read RSM");

        match parse_rsm(&data) {
            Ok((_, rsm)) => {
                assert_eq!(rsm.version, 1.5);
                assert_eq!(rsm.nodes.len(), 2);
            }
            Err(e) => panic!("Failed to parse cherrytree_s_01.rsm: {e:?}"),
        }
    }

    #[test]
    fn test_parse_cherry_tree_02() {
        let data = fs::read("assets/cherrytree_s_02.rsm").expect("Failed to read RSM");

        match parse_rsm(&data) {
            Ok((_, rsm)) => {
                assert_eq!(rsm.version, 1.5);
                assert_eq!(rsm.nodes.len(), 2);
            }
            Err(e) => panic!("Failed to parse cherrytree_s_02.rsm: {e:?}"),
        }
    }

    #[test]
    fn test_parse_cherry_flower_01() {
        let data = fs::read("assets/cherryflower_s_01.rsm").expect("Failed to read RSM");

        match parse_rsm(&data) {
            Ok((_, rsm)) => {
                // Verify parsing succeeded
                assert_eq!(rsm.version, 1.5);
                assert!(!rsm.nodes.is_empty());
            }
            Err(e) => panic!("Failed to parse cherryflower_s_01.rsm: {e:?}"),
        }
    }
}
