use anyhow::bail;
use glam::{Mat4, Quat, Vec3, Vec4};
use ro_formats::{Node, PosKeyframe, RotKeyframe, Rsm};
use std::collections::{BTreeMap, HashMap, HashSet};

/// One RSM node, flattened for the glb writer.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelNode {
    pub name: String,
    pub parent: Option<usize>,
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub pos_keyframes: Vec<PosKeyframe>,
    pub rot_keyframes: Vec<RotKeyframe>,
    pub primitives: Vec<ModelPrimitive>,
}

/// The geometry of one node batched under a single resolved RSM texture id
/// (`-1` when the face referenced a texture slot the node does not have).
#[derive(Debug, Clone, PartialEq)]
pub struct ModelPrimitive {
    pub texture_id: i32,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelBuild {
    pub nodes: Vec<ModelNode>,
}

/// Build the renderable form of an RSM, bevy-free.
///
/// Mirrors the native mesh math in
/// `game-engine/src/presentation/rendering/models.rs`
/// (`extract_node_meshes`, `generate_meshes_from_vertices_and_faces`,
/// `rsm_node_to_bevy_transform`, `find_parent_node_index`, `mat3_to_mat4`)
/// verbatim, quirks included: the node `offset` is baked into the vertices
/// only for multi-node models, `mat3` is always baked, face normals are flat,
/// vertices dedup on `(vertex_id, texture_vertex_id)` keeping the first face's
/// normal, and every completed triangle has its last two indices swapped.
///
/// Two deliberate deviations from the native code (not from the native look):
/// faces group per texture in a `BTreeMap` so the output is byte-stable, and a
/// model that yields no geometry at all is an error instead of the native
/// debug fallback cube.
///
/// Node names are uniquified because glTF animation channels target nodes by
/// name: an empty name becomes `node_<index>`, and a name already taken by an
/// earlier node becomes `<name>_<index>`.
pub fn build_model(rsm: &Rsm) -> anyhow::Result<ModelBuild> {
    let names = uniquify_names(rsm);

    let nodes: Vec<ModelNode> = rsm
        .nodes
        .iter()
        .enumerate()
        .map(|(idx, node)| {
            let (translation, rotation, scale) = node_trs(rsm, node, idx);
            ModelNode {
                name: names[idx].clone(),
                parent: find_parent_node_index(rsm, node),
                translation,
                rotation,
                scale,
                pos_keyframes: node.pos_keyframes.clone(),
                rot_keyframes: node.rot_keyframes.clone(),
                primitives: extract_node_primitives(rsm, node),
            }
        })
        .collect();

    if nodes.iter().all(|n| n.primitives.is_empty()) {
        bail!(
            "RSM produced no geometry: {} node(s), main node {:?}",
            rsm.nodes.len(),
            rsm.main_node_name
        );
    }

    Ok(ModelBuild { nodes })
}

/// Mirrors `models.rs::mat3_to_mat4` - the RSM `mat3` is column-major.
fn mat3_to_mat4(mat3: &[f32; 9]) -> Mat4 {
    Mat4::from_cols(
        Vec4::new(mat3[0], mat3[1], mat3[2], 0.0),
        Vec4::new(mat3[3], mat3[4], mat3[5], 0.0),
        Vec4::new(mat3[6], mat3[7], mat3[8], 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    )
}

/// Mirrors `models.rs::extract_node_meshes`.
fn extract_node_primitives(rsm: &Rsm, node: &Node) -> Vec<ModelPrimitive> {
    if node.vertices.is_empty() || node.faces.is_empty() {
        return Vec::new();
    }

    let is_only = rsm.nodes.len() == 1;
    let mut transform = Mat4::IDENTITY;
    if !is_only {
        transform *= Mat4::from_translation(Vec3::from_array(node.offset));
    }
    transform *= mat3_to_mat4(&node.mat3);

    let transformed: Vec<[f32; 3]> = node
        .vertices
        .iter()
        .map(|v| {
            (transform * Vec4::new(v[0], v[1], v[2], 1.0))
                .truncate()
                .to_array()
        })
        .collect();

    generate_primitives(node, &transformed)
}

/// Mirrors `models.rs::generate_meshes_from_vertices_and_faces`.
fn generate_primitives(node: &Node, transformed: &[[f32; 3]]) -> Vec<ModelPrimitive> {
    let mut faces_by_texture: BTreeMap<i32, Vec<usize>> = BTreeMap::new();
    for (idx, face) in node.faces.iter().enumerate() {
        let texture_id = node
            .texture_ids
            .get(face.tex_id as usize)
            .copied()
            .unwrap_or(-1);
        faces_by_texture.entry(texture_id).or_default().push(idx);
    }

    let face_normals: Vec<[f32; 3]> = node
        .faces
        .iter()
        .map(|f| face_normal(f, transformed))
        .collect();

    faces_by_texture
        .into_iter()
        .filter_map(|(texture_id, face_indices)| {
            build_primitive(node, transformed, &face_normals, texture_id, &face_indices)
        })
        .collect()
}

fn face_normal(face: &ro_formats::Face, transformed: &[[f32; 3]]) -> [f32; 3] {
    let ids = face.vertex_ids.map(|id| id as usize);
    if ids.iter().any(|&id| id >= transformed.len()) {
        return [0.0, 1.0, 0.0];
    }
    let v1 = Vec3::from(transformed[ids[0]]);
    let v2 = Vec3::from(transformed[ids[1]]);
    let v3 = Vec3::from(transformed[ids[2]]);
    (v2 - v1).cross(v3 - v1).normalize_or_zero().to_array()
}

fn build_primitive(
    node: &Node,
    transformed: &[[f32; 3]],
    face_normals: &[[f32; 3]],
    texture_id: i32,
    face_indices: &[usize],
) -> Option<ModelPrimitive> {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut vertex_map: HashMap<(u16, u16), u32> = HashMap::new();

    for &face_idx in face_indices {
        let face = &node.faces[face_idx];
        let face_normal = face_normals[face_idx];

        for corner in 0..3 {
            let pos_idx = face.vertex_ids[corner];
            let uv_idx = face.texture_vertex_ids[corner];
            if pos_idx as usize >= node.vertices.len() {
                continue;
            }

            if let Some(&existing) = vertex_map.get(&(pos_idx, uv_idx)) {
                indices.push(existing);
                continue;
            }

            let uv = node
                .texture_vertices
                .get(uv_idx as usize)
                .map_or([0.0, 0.0], |tv| [tv.u, tv.v]);

            let new_idx = positions.len() as u32;
            positions.push(transformed[pos_idx as usize]);
            uvs.push(uv);
            normals.push(face_normal);
            indices.push(new_idx);
            vertex_map.insert((pos_idx, uv_idx), new_idx);
        }

        let idx_count = indices.len();
        if idx_count >= 3 && idx_count.is_multiple_of(3) {
            indices.swap(idx_count - 2, idx_count - 1);
        }
    }

    if positions.is_empty() {
        return None;
    }

    Some(ModelPrimitive {
        texture_id,
        positions,
        normals,
        uvs,
        indices,
    })
}

/// Mirrors `models.rs::rsm_node_to_bevy_transform`, returning raw-local TRS.
fn node_trs(rsm: &Rsm, node: &Node, node_idx: usize) -> ([f32; 3], [f32; 4], [f32; 3]) {
    let mut translation = Vec3::from_array(node.pos);

    let mut rotation = Quat::IDENTITY;
    if node.rot_angle != 0.0 {
        let axis = Vec3::from_array(node.rot_axis);
        if axis.length() > 0.0 {
            rotation = Quat::from_axis_angle(axis.normalize(), node.rot_angle);
        }
    }

    let is_main_node = node.name == rsm.main_node_name || node_idx == 0;
    if is_main_node && let Some(bbox) = &rsm.bounding_box {
        translation += Vec3::new(-bbox.center[0], -bbox.max[1], -bbox.center[2]);
    }

    (translation.to_array(), rotation.to_array(), node.scale)
}

/// Mirrors `models.rs::find_parent_node_index`.
fn find_parent_node_index(rsm: &Rsm, node: &Node) -> Option<usize> {
    if node.parent_name.is_empty() || node.parent_name == node.name {
        return None;
    }
    rsm.nodes.iter().position(|n| n.name == node.parent_name)
}

fn uniquify_names(rsm: &Rsm) -> Vec<String> {
    let mut taken: HashSet<String> = HashSet::new();
    rsm.nodes
        .iter()
        .enumerate()
        .map(|(idx, node)| {
            let base = if node.name.is_empty() {
                format!("node_{idx}")
            } else {
                node.name.clone()
            };
            let mut candidate = base.clone();
            let mut attempt = 0;
            while taken.contains(&candidate) {
                attempt += 1;
                candidate = match attempt {
                    1 => format!("{base}_{idx}"),
                    n => format!("{base}_{idx}_{n}"),
                };
            }
            taken.insert(candidate.clone());
            candidate
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ro_formats::{BoundingBox, Face, ShadingType, TextureVertex};

    fn face(vertex_ids: [u16; 3], texture_vertex_ids: [u16; 3]) -> Face {
        Face {
            vertex_ids,
            texture_vertex_ids,
            tex_id: 0,
            padding: 0,
            two_side: 0,
            smooth_group: 0,
        }
    }

    fn uv(u: f32, v: f32) -> TextureVertex {
        TextureVertex { color: None, u, v }
    }

    fn node(name: &str) -> Node {
        Node {
            name: name.to_string(),
            parent_name: String::new(),
            texture_ids: vec![3],
            mat3: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            offset: [0.0, 0.0, 0.0],
            pos: [0.0, 0.0, 0.0],
            rot_angle: 0.0,
            rot_axis: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            vertices: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            texture_vertices: vec![uv(0.0, 0.0), uv(1.0, 0.0), uv(0.0, 1.0)],
            faces: vec![face([0, 1, 2], [0, 1, 2])],
            pos_keyframes: Vec::new(),
            rot_keyframes: Vec::new(),
        }
    }

    fn rsm(nodes: Vec<Node>) -> Rsm {
        let main_node_name = nodes[0].name.clone();
        Rsm {
            version: 1.4,
            anim_len: 0,
            shade_type: ShadingType::Smooth,
            alpha: 1.0,
            textures: vec!["a.bmp".into(), "b.bmp".into()],
            main_node_name,
            nodes,
            pos_keyframes: Vec::new(),
            volume_boxes: Vec::new(),
            bounding_box: None,
        }
    }

    #[test]
    fn single_node_bakes_mat3_without_the_offset() {
        let mut n = node("main");
        n.offset = [10.0, 20.0, 30.0];
        n.mat3 = [2.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 2.0];

        let build = build_model(&rsm(vec![n])).expect("model must build");

        let prim = &build.nodes[0].primitives[0];
        assert_eq!(prim.texture_id, 3);
        assert_eq!(
            prim.positions,
            vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]]
        );
        assert_eq!(prim.uvs, vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]);
        assert_eq!(prim.normals, vec![[0.0, 0.0, 1.0]; 3]);
    }

    #[test]
    fn multi_node_bakes_offset_then_column_major_mat3() {
        let mut main = node("main");
        main.offset = [10.0, 20.0, 30.0];
        main.mat3 = [0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

        let mut child = node("child");
        child.parent_name = "main".into();
        child.vertices = Vec::new();
        child.faces = Vec::new();

        let build = build_model(&rsm(vec![main, child])).expect("model must build");

        assert_eq!(
            build.nodes[0].primitives[0].positions,
            vec![[10.0, 20.0, 30.0], [10.0, 21.0, 30.0], [9.0, 20.0, 30.0]]
        );
        assert!(build.nodes[1].primitives.is_empty());
    }

    #[test]
    fn shared_pairs_dedup_and_each_triangle_swaps_its_last_two_indices() {
        let mut n = node("main");
        n.vertices = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
        ];
        n.texture_vertices = vec![
            uv(0.0, 0.0),
            uv(1.0, 0.0),
            uv(0.0, 1.0),
            uv(1.0, 1.0),
            uv(0.5, 0.5),
        ];
        n.faces = vec![face([0, 1, 2], [0, 1, 2]), face([2, 1, 3], [2, 4, 3])];

        let build = build_model(&rsm(vec![n])).expect("model must build");
        let prim = &build.nodes[0].primitives[0];

        assert_eq!(
            prim.positions,
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 1.0],
            ]
        );
        assert_eq!(
            prim.uvs,
            vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [0.5, 0.5], [1.0, 1.0]]
        );

        let second_normal = Vec3::new(-1.0, -1.0, 1.0).normalize().to_array();
        assert_eq!(
            prim.normals,
            vec![
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
                second_normal,
                second_normal,
            ]
        );
        assert_eq!(prim.indices, vec![0, 2, 1, 2, 4, 3]);
    }

    #[test]
    fn out_of_range_ids_skip_the_corner_fall_back_to_zero_uv_and_up_normal() {
        let mut n = node("main");
        n.faces = vec![face([0, 1, 5], [0, 9, 2]), face([0, 1, 2], [0, 1, 2])];

        let build = build_model(&rsm(vec![n])).expect("model must build");
        let prim = &build.nodes[0].primitives[0];

        assert_eq!(
            prim.positions,
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0]
            ]
        );
        assert_eq!(
            prim.uvs,
            vec![[0.0, 0.0], [0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]
        );
        assert_eq!(
            prim.normals,
            vec![
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0]
            ]
        );
        assert_eq!(prim.indices, vec![0, 1, 0, 2, 3]);
    }

    #[test]
    fn faces_group_per_resolved_texture_id_in_ascending_order() {
        let mut n = node("main");
        n.texture_ids = vec![7, 2];
        n.faces = vec![
            face([0, 1, 2], [0, 1, 2]),
            Face {
                tex_id: 1,
                ..face([0, 1, 2], [0, 1, 2])
            },
            Face {
                tex_id: 9,
                ..face([0, 1, 2], [0, 1, 2])
            },
        ];

        let build = build_model(&rsm(vec![n])).expect("model must build");

        let ids: Vec<i32> = build.nodes[0]
            .primitives
            .iter()
            .map(|p| p.texture_id)
            .collect();
        assert_eq!(ids, vec![-1, 2, 7]);
    }

    #[test]
    fn main_node_gets_the_bbox_offset_and_children_keep_raw_local_trs() {
        let mut main = node("main");
        main.pos = [10.0, 10.0, 10.0];
        main.rot_angle = std::f32::consts::FRAC_PI_2;
        main.rot_axis = [0.0, 2.0, 0.0];
        main.scale = [2.0, 3.0, 4.0];

        let mut child = node("child");
        child.parent_name = "main".into();
        child.pos = [1.0, 1.0, 1.0];

        let mut orphan = node("orphan");
        orphan.parent_name = "orphan".into();

        let mut model = rsm(vec![main, child, orphan]);
        model.bounding_box = Some(BoundingBox {
            min: [0.0, 0.0, 0.0],
            max: [4.0, 5.0, 6.0],
            center: [1.0, 2.0, 3.0],
            range: [2.0, 2.5, 3.0],
        });

        let build = build_model(&model).expect("model must build");

        assert_eq!(build.nodes[0].translation, [9.0, 5.0, 7.0]);
        assert_eq!(
            build.nodes[0].rotation,
            Quat::from_axis_angle(Vec3::Y, std::f32::consts::FRAC_PI_2).to_array()
        );
        assert_eq!(build.nodes[0].scale, [2.0, 3.0, 4.0]);
        assert_eq!(build.nodes[0].parent, None);

        assert_eq!(build.nodes[1].translation, [1.0, 1.0, 1.0]);
        assert_eq!(build.nodes[1].rotation, Quat::IDENTITY.to_array());
        assert_eq!(build.nodes[1].parent, Some(0));

        assert_eq!(build.nodes[2].parent, None);
    }

    #[test]
    fn a_degenerate_rotation_axis_leaves_the_rotation_identity() {
        let mut n = node("main");
        n.rot_angle = 1.0;
        n.rot_axis = [0.0, 0.0, 0.0];

        let build = build_model(&rsm(vec![n])).expect("model must build");

        assert_eq!(build.nodes[0].rotation, Quat::IDENTITY.to_array());
    }

    #[test]
    fn duplicate_and_empty_node_names_are_uniquified() {
        let mut second = node("dup");
        second.parent_name = "dup".into();
        let mut third = node("");
        third.parent_name = "dup".into();

        let build = build_model(&rsm(vec![node("dup"), second, third])).expect("model must build");

        let names: Vec<&str> = build.nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, vec!["dup", "dup_1", "node_2"]);
        assert_eq!(build.nodes[1].parent, None);
        assert_eq!(build.nodes[2].parent, Some(0));
    }

    #[test]
    fn a_model_without_geometry_fails_loudly() {
        let mut n = node("main");
        n.faces = Vec::new();

        let err = build_model(&rsm(vec![n])).expect_err("geometry-less model must fail");

        assert!(
            err.to_string().contains("no geometry"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn two_builds_of_the_same_rsm_are_identical() {
        let mut n = node("main");
        n.texture_ids = vec![7, 2, 5];
        n.faces = vec![
            face([0, 1, 2], [0, 1, 2]),
            Face {
                tex_id: 1,
                ..face([2, 1, 0], [2, 1, 0])
            },
            Face {
                tex_id: 2,
                ..face([1, 2, 0], [1, 2, 0])
            },
        ];
        n.pos_keyframes = vec![PosKeyframe {
            frame: 3,
            px: 1.0,
            py: 2.0,
            pz: 3.0,
        }];
        n.rot_keyframes = vec![RotKeyframe {
            frame: 5,
            q: [0.0, 0.0, 0.0, 1.0],
        }];
        let model = rsm(vec![n]);

        let first = build_model(&model).expect("model must build");
        let second = build_model(&model).expect("model must build");

        assert_eq!(first, second);
        assert_eq!(first.nodes[0].pos_keyframes, model.nodes[0].pos_keyframes);
        assert_eq!(first.nodes[0].rot_keyframes, model.nodes[0].rot_keyframes);
    }
}
