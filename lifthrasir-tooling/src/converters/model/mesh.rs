use crate::converters::model::normalized::{
    AlphaMode, ModelProvenance, NormalizedKey, NormalizedMaterial, NormalizedModel, NormalizedNode,
    NormalizedPrimitive, NormalizedTrack, ShadingPolicy,
};
use anyhow::{bail, ensure};
use glam::{Mat4, Quat, Vec3, Vec4};
use ro_formats::{Node, Rsm, ShadingType};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

const RSM1_ALPHA_CUTOFF: f32 = 0.01;

#[derive(Debug, Clone, PartialEq)]
struct Rsm1Primitive {
    texture_id: i32,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
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
pub fn build_model(rsm: &Rsm, source_hash: &str) -> anyhow::Result<NormalizedModel> {
    let names = uniquify_names(rsm);
    let raw_primitives: Vec<Vec<Rsm1Primitive>> = rsm
        .nodes
        .iter()
        .map(|node| extract_node_primitives(rsm, node))
        .collect();

    if raw_primitives.iter().all(Vec::is_empty) {
        bail!(
            "RSM produced no geometry: {} node(s), main node {:?}",
            rsm.nodes.len(),
            rsm.main_node_name
        );
    }

    let texture_ids: BTreeSet<i32> = raw_primitives
        .iter()
        .flatten()
        .map(|primitive| primitive.texture_id)
        .collect();
    let material_by_texture: BTreeMap<i32, usize> = texture_ids
        .iter()
        .enumerate()
        .map(|(material, texture)| (*texture, material))
        .collect();
    let alpha = if rsm.alpha < 1.0 {
        AlphaMode::Blend
    } else {
        AlphaMode::Mask {
            cutoff: RSM1_ALPHA_CUTOFF,
        }
    };
    let shading = match rsm.shade_type {
        // Phase 2 rendered every RSM1 material lit; preserve that boundary.
        // RSM2's true no-shade policy is normalized by its own adapter.
        ShadingType::None | ShadingType::Flat => ShadingPolicy::Flat,
        ShadingType::Smooth => ShadingPolicy::Smooth,
    };
    let materials = texture_ids
        .into_iter()
        .map(|texture| NormalizedMaterial {
            texture: usize::try_from(texture).ok(),
            alpha,
            two_sided: true,
            shading,
        })
        .collect();

    let duration_ms = rsm.anim_len.max(0) as f32;
    let nodes: Vec<NormalizedNode> = rsm
        .nodes
        .iter()
        .enumerate()
        .map(|(idx, node)| {
            let (translation, rotation, scale) = node_trs(rsm, node, idx);
            Ok(NormalizedNode {
                name: names[idx].clone(),
                parent: find_parent_node_index(rsm, node),
                translation,
                rotation,
                scale,
                matrix: None,
                // RSM1 has no per-node translation animation - position
                // keyframes only exist from RSM2 onwards, and `rsm2.rs`
                // handles those.
                translation_track: NormalizedTrack::default(),
                rotation_track: rotation_track(
                    &names[idx],
                    duration_ms,
                    node.rot_keyframes.iter().map(|key| (key.frame, key.q)),
                )?,
                scale_track: NormalizedTrack::default(),
                primitives: raw_primitives[idx]
                    .iter()
                    .map(|primitive| NormalizedPrimitive {
                        material: material_by_texture[&primitive.texture_id],
                        positions: primitive.positions.clone(),
                        normals: primitive.normals.clone(),
                        uv0: primitive.uvs.clone(),
                        uv1: None,
                        indices: primitive.indices.clone(),
                        uv_animation: None,
                    })
                    .collect(),
            })
        })
        .collect::<anyhow::Result<_>>()?;
    let roots = nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| node.parent.is_none().then_some(index))
        .collect();

    Ok(NormalizedModel {
        duration_ms,
        textures: rsm.textures.clone(),
        roots,
        nodes,
        materials,
        provenance: ModelProvenance {
            source_version: format!("{:.1}", rsm.version),
            source_hash: source_hash.to_string(),
        },
    })
}

fn rotation_track(
    node_name: &str,
    duration_ms: f32,
    source: impl IntoIterator<Item = (i32, [f32; 4])>,
) -> anyhow::Result<NormalizedTrack<[f32; 4]>> {
    if duration_ms <= 0.0 {
        return Ok(NormalizedTrack::default());
    }

    let source: Vec<_> = source.into_iter().collect();
    let max_frame = source.iter().map(|(frame, _)| *frame).max().unwrap_or(0);
    let duration_seconds = duration_ms / 1000.0;
    let keys = source
        .into_iter()
        .map(|(frame, value)| NormalizedKey {
            time_ms: if max_frame == 0 {
                0.0
            } else {
                frame as f32 / max_frame as f32 * duration_seconds * 1000.0
            },
            value,
        })
        .collect::<Vec<_>>();
    ensure!(
        !keys
            .windows(2)
            .any(|pair| pair[1].time_ms <= pair[0].time_ms),
        "node '{node_name}' has non-increasing rotation keyframes"
    );
    let mut keys = fold_pre_roll(node_name, keys)?;
    if keys.len() == 1 && keys[0].time_ms == 0.0 {
        keys.push(NormalizedKey {
            time_ms: duration_ms,
            value: keys[0].value,
        });
    }
    Ok(NormalizedTrack { keys })
}

/// Collapses keyframes placed before the start of the animation onto a single
/// key at `t = 0`.
///
/// A handful of retail RSM1 nodes (`아인브로크\용광로06.rsm`'s `Object01`
/// opens at frame -160) carry pre-roll keys. Playback time never goes
/// negative, so those keys only ever act as the interpolation source for the
/// first stretch of the loop -- which is exactly the pose this reproduces,
/// while glTF, which forbids negative key times, gets a track it accepts.
fn fold_pre_roll(
    node_name: &str,
    keys: Vec<NormalizedKey<[f32; 4]>>,
) -> anyhow::Result<Vec<NormalizedKey<[f32; 4]>>> {
    let played_from = keys.iter().position(|key| key.time_ms >= 0.0);
    let Some(played_from) = played_from else {
        // Times are strictly increasing, so this means every key precedes the
        // animation and the node has no pose to play at all.
        ensure!(
            keys.is_empty(),
            "node '{node_name}' has only pre-roll rotation keyframes"
        );
        return Ok(keys);
    };
    if played_from == 0 {
        return Ok(keys);
    }

    let last_pre_roll = &keys[played_from - 1];
    let first_played = &keys[played_from];
    let start = if first_played.time_ms == 0.0 {
        first_played.value
    } else {
        let span = first_played.time_ms - last_pre_roll.time_ms;
        let factor = -last_pre_roll.time_ms / span;
        // The t=0 pose is synthesized, so unlike the source keys -- which pass
        // through verbatim -- it is normalized: `slerp` is only defined on unit
        // quaternions and returns NaN outside them.
        Quat::from_array(last_pre_roll.value)
            .normalize()
            .slerp(Quat::from_array(first_played.value).normalize(), factor)
            .to_array()
    };

    let mut folded = vec![NormalizedKey {
        time_ms: 0.0,
        value: start,
    }];
    folded.extend(
        keys.into_iter()
            .skip(played_from)
            .filter(|key| key.time_ms > 0.0),
    );
    Ok(folded)
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
fn extract_node_primitives(rsm: &Rsm, node: &Node) -> Vec<Rsm1Primitive> {
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
fn generate_primitives(node: &Node, transformed: &[[f32; 3]]) -> Vec<Rsm1Primitive> {
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
) -> Option<Rsm1Primitive> {
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

    Some(Rsm1Primitive {
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
    use ro_formats::{BoundingBox, Face, RotKeyframe, ShadingType, TextureVertex};

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
            rot_keyframes: Vec::new(),
        }
    }

    fn rsm(nodes: Vec<Node>) -> Rsm {
        let main_node_name = nodes[0].name.clone();
        Rsm {
            version: 1.4,
            raw_version: 0x0104,
            anim_len: 0,
            shade_type: ShadingType::Smooth,
            alpha: 1.0,
            textures: vec!["a.bmp".into(), "b.bmp".into()],
            main_node_name,
            nodes,
            scale_keyframes: Vec::new(),
            volume_boxes: Vec::new(),
            bounding_box: None,
        }
    }

    #[test]
    fn single_node_bakes_mat3_without_the_offset() {
        let mut n = node("main");
        n.offset = [10.0, 20.0, 30.0];
        n.mat3 = [2.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 2.0];

        let build = build_model(&rsm(vec![n]), "hash").expect("model must build");

        let prim = &build.nodes[0].primitives[0];
        assert_eq!(prim.material, 0);
        assert_eq!(build.materials[prim.material].texture, Some(3));
        assert_eq!(
            prim.positions,
            vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]]
        );
        assert_eq!(prim.uv0, vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]);
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

        let build = build_model(&rsm(vec![main, child]), "hash").expect("model must build");

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

        let build = build_model(&rsm(vec![n]), "hash").expect("model must build");
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
            prim.uv0,
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

        let build = build_model(&rsm(vec![n]), "hash").expect("model must build");
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
            prim.uv0,
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

        let build = build_model(&rsm(vec![n]), "hash").expect("model must build");

        let ids: Vec<Option<usize>> = build
            .materials
            .iter()
            .map(|material| material.texture)
            .collect();
        assert_eq!(ids, vec![None, Some(2), Some(7)]);
        assert_eq!(
            build.nodes[0]
                .primitives
                .iter()
                .map(|primitive| primitive.material)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
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

        let build = build_model(&model, "hash").expect("model must build");

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

        let build = build_model(&rsm(vec![n]), "hash").expect("model must build");

        assert_eq!(build.nodes[0].rotation, Quat::IDENTITY.to_array());
    }

    #[test]
    fn duplicate_and_empty_node_names_are_uniquified() {
        let mut second = node("dup");
        second.parent_name = "dup".into();
        let mut third = node("");
        third.parent_name = "dup".into();

        let build =
            build_model(&rsm(vec![node("dup"), second, third]), "hash").expect("model must build");

        let names: Vec<&str> = build.nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, vec!["dup", "dup_1", "node_2"]);
        assert_eq!(build.nodes[1].parent, None);
        assert_eq!(build.nodes[2].parent, Some(0));
    }

    #[test]
    fn a_model_without_geometry_fails_loudly() {
        let mut n = node("main");
        n.faces = Vec::new();

        let err = build_model(&rsm(vec![n]), "hash").expect_err("geometry-less model must fail");

        assert!(
            err.to_string().contains("no geometry"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn one_time_zero_key_covers_duration_and_rsm1_none_stays_lit() {
        let mut source_node = node("main");
        source_node.rot_keyframes.push(RotKeyframe {
            frame: 0,
            q: [0.0, 0.0, 0.0, 1.0],
        });
        let mut source = rsm(vec![source_node]);
        source.anim_len = 1_000;
        source.shade_type = ShadingType::None;

        let model = build_model(&source, "hash").unwrap();

        assert_eq!(
            model.nodes[0]
                .rotation_track
                .keys
                .iter()
                .map(|key| key.time_ms)
                .collect::<Vec<_>>(),
            vec![0.0, 1_000.0]
        );
        assert_eq!(model.materials[0].shading, ShadingPolicy::Flat);
    }

    /// `아인브로크\용광로06.rsm`'s `Object01` opens at frame -160 of a 25600ms
    /// animation. Playback never reaches a negative time, and glTF forbids
    /// one, so the pre-roll collapses into the pose it interpolates to at t=0.
    #[test]
    fn pre_roll_keyframes_collapse_into_the_pose_at_time_zero() {
        let quarter_turn = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let mut source_node = node("main");
        source_node.rot_keyframes = vec![
            RotKeyframe {
                frame: -500,
                q: Quat::IDENTITY.to_array(),
            },
            RotKeyframe {
                frame: 500,
                q: quarter_turn.to_array(),
            },
            RotKeyframe {
                frame: 1_000,
                q: Quat::IDENTITY.to_array(),
            },
        ];
        let mut source = rsm(vec![source_node]);
        source.anim_len = 1_000;

        let keys = build_model(&source, "hash").unwrap().nodes[0]
            .rotation_track
            .keys
            .clone();

        assert_eq!(
            keys.iter().map(|key| key.time_ms).collect::<Vec<_>>(),
            vec![0.0, 500.0, 1_000.0]
        );
        let expected = Quat::IDENTITY.slerp(quarter_turn, 0.5);
        assert!(
            Quat::from_array(keys[0].value).abs_diff_eq(expected, 1e-6),
            "t=0 must be the pre-roll interpolated forward, got {:?}",
            keys[0].value
        );
    }

    /// A pre-roll key landing exactly on t=0 is already the played pose, so it
    /// replaces the pre-roll rather than being interpolated against.
    #[test]
    fn a_pre_roll_key_meeting_time_zero_keeps_the_time_zero_pose() {
        let quarter_turn = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let mut source_node = node("main");
        source_node.rot_keyframes = vec![
            RotKeyframe {
                frame: -500,
                q: Quat::IDENTITY.to_array(),
            },
            RotKeyframe {
                frame: 0,
                q: quarter_turn.to_array(),
            },
            RotKeyframe {
                frame: 1_000,
                q: Quat::IDENTITY.to_array(),
            },
        ];
        let mut source = rsm(vec![source_node]);
        source.anim_len = 1_000;

        let keys = build_model(&source, "hash").unwrap().nodes[0]
            .rotation_track
            .keys
            .clone();

        assert_eq!(
            keys.iter().map(|key| key.time_ms).collect::<Vec<_>>(),
            vec![0.0, 1_000.0]
        );
        assert_eq!(keys[0].value, quarter_turn.to_array());
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
        n.rot_keyframes = vec![RotKeyframe {
            frame: 3,
            q: [0.0, 0.0, 0.0, 1.0],
        }];
        let mut model = rsm(vec![n]);
        model.anim_len = 1000;

        let first = build_model(&model, "hash").expect("model must build");
        let second = build_model(&model, "hash").expect("model must build");

        assert_eq!(first, second);
        assert_eq!(first.nodes[0].rotation_track.keys[0].time_ms, 1000.0);
        assert_eq!(
            first.nodes[0].rotation_track.keys[0].value,
            [0.0, 0.0, 0.0, 1.0]
        );
    }
}
