use crate::converters::model::normalized::{
    AlphaMode, ModelProvenance, NormalizedKey, NormalizedMaterial, NormalizedModel, NormalizedNode,
    NormalizedPrimitive, NormalizedTrack, ShadingPolicy,
};
use anyhow::{Context, bail, ensure};
use glam::{Mat3, Mat4, Quat, Vec2, Vec3};
use lifthrasir_data::lif::{
    LifScalarKey, LifUvAnimation, LifUvChannel, LifUvProperty, LifUvSample,
};
use ro_formats::rsm2::{
    Rsm2, Rsm2Node, Rsm2NodeTextures, Rsm2TextureAnimation, Rsm2TextureChannelType, Rsm2Version,
};
use std::collections::{BTreeMap, HashMap, HashSet};

const MATRIX_EPSILON: f32 = 1.0e-4;
const SOURCE_BASIS: Mat3 = Mat3::from_cols_array(&[1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 1.0]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct MaterialKey {
    texture: usize,
    two_sided: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct PrimitiveKey {
    material: MaterialKey,
    uv_target: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CornerKey {
    position: usize,
    uv: usize,
    normal_identity: i64,
}

struct RawPrimitive {
    key: PrimitiveKey,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

pub fn build_model(source: &Rsm2, source_hash: &str) -> anyhow::Result<NormalizedModel> {
    ensure!(
        source.frames_per_second.is_finite() && source.frames_per_second > 0.0,
        "RSM2 frames per second must be positive and finite"
    );
    ensure!(
        source.animation_length >= 0,
        "negative RSM2 animation length"
    );
    let duration_ms = source_time(source.animation_length, source.frames_per_second)?;
    let duration_u32 = exact_u32(duration_ms, "animation duration")?;
    let shading = match source.shade_type {
        0 => ShadingPolicy::None,
        1 => ShadingPolicy::Flat,
        2 => ShadingPolicy::Smooth,
        value => bail!("unsupported RSM2 shade type {value}"),
    };

    let (textures, node_textures) = resolve_textures(source)?;
    // Names are not unique in retail data, so the hierarchy is resolved by index
    // using the same first-match rule the parser validated against.
    let parents =
        ro_formats::parent_indices(&source.nodes).context("resolving the node hierarchy")?;
    let roots = ro_formats::root_indices(&source.roots, &source.nodes);

    let mut raw_nodes = Vec::with_capacity(source.nodes.len());
    let mut material_keys = BTreeMap::<MaterialKey, usize>::new();
    for (index, node) in source.nodes.iter().enumerate() {
        let transforms = node_transforms(source, index, parents[index], duration_ms)?;
        let animations = texture_animations(node, duration_u32, source.frames_per_second)?;
        let primitives = geometry(node, &node_textures[index], &textures, &animations, shading)?;
        for primitive in &primitives {
            material_keys.entry(primitive.key.material).or_default();
        }
        raw_nodes.push((transforms, animations, primitives));
    }
    ensure!(
        raw_nodes
            .iter()
            .any(|(_, _, primitives)| !primitives.is_empty()),
        "RSM2 produced no geometry"
    );

    for (index, value) in material_keys.values_mut().enumerate() {
        *value = index;
    }
    let materials = material_keys
        .keys()
        .map(|key| NormalizedMaterial {
            texture: Some(key.texture),
            alpha: alpha_mode(&textures[key.texture]),
            two_sided: key.two_sided,
            shading,
        })
        .collect();

    let normalized_primitives = raw_nodes
        .iter()
        .map(|(_, animations, raw_primitives)| {
            raw_primitives
                .iter()
                .map(|raw| {
                    let uv_animation = raw
                        .key
                        .uv_target
                        .and_then(|target| animations.get(&target))
                        .cloned();
                    let (uv0, uv1) = if let Some(animation) = &uv_animation {
                        let sample = animation.sample(0, false).map_err(anyhow::Error::msg)?;
                        (
                            raw.uvs.iter().map(|uv| transform_uv(*uv, sample)).collect(),
                            Some(raw.uvs.clone()),
                        )
                    } else {
                        (raw.uvs.clone(), None)
                    };
                    Ok(NormalizedPrimitive {
                        material: material_keys[&raw.key.material],
                        positions: raw.positions.clone(),
                        normals: raw.normals.clone(),
                        uv0,
                        uv1,
                        indices: raw.indices.clone(),
                        uv_animation,
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let mut logical_indices = Vec::with_capacity(source.nodes.len());
    let mut tail_indices = Vec::with_capacity(source.nodes.len());
    let mut node_count = 0;
    for (transforms, _, _) in &raw_nodes {
        logical_indices.push(node_count);
        node_count += 1;
        if transforms.matrix.is_some() {
            node_count += 1;
            if !transforms.scale_track.keys.is_empty() {
                node_count += 1;
            }
        }
        tail_indices.push(node_count - 1);
    }

    // Retail models repeat node names, but glTF animation channels target a node
    // by name, so the normalized model requires them to be unique. The hierarchy
    // was already resolved by index above, so renaming here changes nothing
    // structural - it only keeps every node addressable.
    let names = uniquify_node_names(&source.nodes);
    let mut used_names = names.iter().cloned().collect::<HashSet<_>>();
    let mut nodes = Vec::with_capacity(node_count);
    for (index, primitives) in normalized_primitives.into_iter().enumerate() {
        let transforms = &raw_nodes[index].0;
        let parent = parents[index].map(|parent| tail_indices[parent]);
        if let Some(matrix) = transforms.matrix {
            ensure!(
                transforms.rotation_track.keys.is_empty(),
                "matrix helper cannot carry rotation animation"
            );
            nodes.push(NormalizedNode {
                name: names[index].clone(),
                parent,
                translation: transforms.translation,
                rotation: Quat::IDENTITY.to_array(),
                scale: Vec3::ONE.to_array(),
                matrix: None,
                translation_track: transforms.translation_track.clone(),
                rotation_track: NormalizedTrack::default(),
                scale_track: NormalizedTrack::default(),
                primitives: Vec::new(),
            });
            let matrix_index = nodes.len();
            nodes.push(NormalizedNode {
                name: helper_name(&mut used_names, index, "matrix"),
                parent: Some(logical_indices[index]),
                translation: Vec3::ZERO.to_array(),
                rotation: Quat::IDENTITY.to_array(),
                scale: Vec3::ONE.to_array(),
                matrix: Some(matrix),
                translation_track: NormalizedTrack::default(),
                rotation_track: NormalizedTrack::default(),
                scale_track: NormalizedTrack::default(),
                primitives: if transforms.scale_track.keys.is_empty() {
                    primitives.clone()
                } else {
                    Vec::new()
                },
            });
            if !transforms.scale_track.keys.is_empty() {
                nodes.push(NormalizedNode {
                    name: helper_name(&mut used_names, index, "scale"),
                    parent: Some(matrix_index),
                    translation: Vec3::ZERO.to_array(),
                    rotation: Quat::IDENTITY.to_array(),
                    scale: Vec3::ONE.to_array(),
                    matrix: None,
                    translation_track: NormalizedTrack::default(),
                    rotation_track: NormalizedTrack::default(),
                    scale_track: transforms.scale_track.clone(),
                    primitives,
                });
            }
        } else {
            nodes.push(NormalizedNode {
                name: names[index].clone(),
                parent,
                translation: transforms.translation,
                rotation: transforms.rotation,
                scale: transforms.scale,
                matrix: None,
                translation_track: transforms.translation_track.clone(),
                rotation_track: transforms.rotation_track.clone(),
                scale_track: transforms.scale_track.clone(),
                primitives,
            });
        }
    }
    let roots = roots.iter().map(|index| logical_indices[*index]).collect();

    let model = NormalizedModel {
        duration_ms,
        textures,
        roots,
        nodes,
        materials,
        provenance: ModelProvenance {
            source_version: match source.version {
                Rsm2Version::V2_2 => "2.2",
                Rsm2Version::V2_3 => "2.3",
            }
            .to_owned(),
            source_hash: source_hash.to_owned(),
        },
    };
    super::validate::validate_contract(&model)?;
    Ok(model)
}

/// Give every node a unique, non-empty name, mirroring the RSM1 rule: an empty
/// name becomes `node_<index>`, and a name an earlier node already took becomes
/// `<name>_<index>`.
fn uniquify_node_names(nodes: &[ro_formats::Rsm2Node]) -> Vec<String> {
    let mut taken: HashSet<String> = HashSet::new();
    nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let base = if node.name.is_empty() {
                format!("node_{index}")
            } else {
                node.name.clone()
            };
            let mut candidate = base.clone();
            let mut attempt = 0;
            while taken.contains(&candidate) {
                attempt += 1;
                candidate = match attempt {
                    1 => format!("{base}_{index}"),
                    n => format!("{base}_{index}_{n}"),
                };
            }
            taken.insert(candidate.clone());
            candidate
        })
        .collect()
}

fn helper_name(used: &mut HashSet<String>, index: usize, role: &str) -> String {
    let base = format!("__rsm2_{role}_{index}");
    let mut name = base.clone();
    let mut suffix = 1;
    while !used.insert(name.clone()) {
        name = format!("{base}_{suffix}");
        suffix += 1;
    }
    name
}

fn resolve_textures(source: &Rsm2) -> anyhow::Result<(Vec<String>, Vec<Vec<usize>>)> {
    match source.version {
        Rsm2Version::V2_2 => {
            let nodes = source
                .nodes
                .iter()
                .map(|node| match &node.textures {
                    Rsm2NodeTextures::GlobalIndices(indices) => Ok(indices.clone()),
                    Rsm2NodeTextures::Names(_) => {
                        bail!("RSM2 2.2 node contains local texture names")
                    }
                })
                .collect::<anyhow::Result<_>>()?;
            Ok((source.global_textures.clone(), nodes))
        }
        Rsm2Version::V2_3 => {
            let mut textures = Vec::new();
            let mut indices = HashMap::<String, usize>::new();
            let mut nodes = Vec::with_capacity(source.nodes.len());
            for node in &source.nodes {
                let Rsm2NodeTextures::Names(names) = &node.textures else {
                    bail!("RSM2 2.3 node contains global texture indices");
                };
                let mut local = Vec::with_capacity(names.len());
                for name in names {
                    let index = if let Some(index) = indices.get(name) {
                        *index
                    } else {
                        let index = textures.len();
                        textures.push(name.clone());
                        indices.insert(name.clone(), index);
                        index
                    };
                    local.push(index);
                }
                nodes.push(local);
            }
            Ok((textures, nodes))
        }
    }
}

struct NodeTransforms {
    translation: [f32; 3],
    rotation: [f32; 4],
    scale: [f32; 3],
    matrix: Option<[f32; 16]>,
    translation_track: NormalizedTrack<[f32; 3]>,
    rotation_track: NormalizedTrack<[f32; 4]>,
    scale_track: NormalizedTrack<[f32; 3]>,
}

fn node_transforms(
    source: &Rsm2,
    index: usize,
    parent: Option<usize>,
    duration_ms: f32,
) -> anyhow::Result<NodeTransforms> {
    let node = &source.nodes[index];
    let offset_active = node.rotation_keys.is_empty();
    let (offset_scale, offset_rotation, matrix) = if offset_active {
        let offset = offset_matrix(node)?;
        let local_offset = if let Some(parent) = parent {
            offset_matrix(&source.nodes[parent])?.inverse() * offset
        } else {
            offset
        };
        ensure!(
            local_offset.is_finite(),
            "node '{}' has singular parent offset",
            node.name
        );
        let local_offset = SOURCE_BASIS * local_offset * SOURCE_BASIS;
        match decompose_basis(local_offset) {
            Ok((scale, rotation)) => (scale, rotation, None),
            Err(_) => (
                Vec3::ONE,
                Quat::IDENTITY,
                Some(Mat4::from_mat3(local_offset).to_cols_array()),
            ),
        }
    } else {
        (Vec3::ONE, Quat::IDENTITY, None)
    };

    let unkeyed_position = if let Some(parent) = parent {
        let delta = Vec3::from_array(node.offset_position)
            - Vec3::from_array(source.nodes[parent].offset_position);
        SOURCE_BASIS * (offset_matrix(&source.nodes[parent])?.inverse() * delta)
    } else {
        SOURCE_BASIS * Vec3::from_array(node.offset_position)
    };
    let keyed_base_position = SOURCE_BASIS * Vec3::from_array(node.offset_position);
    let translation = if node.position_keys.is_empty() {
        unkeyed_position
    } else {
        keyed_base_position
    };
    let rotation = if offset_active {
        offset_rotation
    } else {
        Quat::IDENTITY
    };
    let scale = if offset_active {
        offset_scale
    } else {
        Vec3::ONE
    };

    let translation_track = boundary_track(
        &node.name,
        "translation",
        duration_ms,
        translation.to_array(),
        node.position_keys.iter().map(|key| {
            source_key(
                key.time,
                source.frames_per_second,
                (SOURCE_BASIS * Vec3::from_array(key.position)).to_array(),
            )
        }),
        |a, b, factor| {
            Vec3::from_array(*a)
                .lerp(Vec3::from_array(*b), factor)
                .to_array()
        },
    )?;
    let rotation_track = boundary_track(
        &node.name,
        "rotation",
        duration_ms,
        Quat::IDENTITY.to_array(),
        node.rotation_keys.iter().map(|key| {
            let quaternion = Quat::from_array(key.quaternion_xyzw);
            ensure!(
                quaternion.is_finite() && quaternion.length_squared() > f32::EPSILON,
                "node '{}' has invalid rotation quaternion",
                node.name
            );
            let matrix = SOURCE_BASIS * Mat3::from_quat(quaternion.normalize()) * SOURCE_BASIS;
            source_key(
                key.time,
                source.frames_per_second,
                Quat::from_mat3(&matrix).normalize().to_array(),
            )
        }),
        |a, b, factor| {
            Quat::from_array(*a)
                .slerp(Quat::from_array(*b), factor)
                .normalize()
                .to_array()
        },
    )?;
    let scale_track = boundary_track(
        &node.name,
        "scale",
        duration_ms,
        scale.to_array(),
        node.scale_keys.iter().map(|key| {
            let keyed = Vec3::from_array(key.scale);
            let value = if offset_active && matrix.is_none() {
                offset_scale * keyed
            } else {
                keyed
            };
            source_key(key.time, source.frames_per_second, value.to_array())
        }),
        |a, b, factor| {
            Vec3::from_array(*a)
                .lerp(Vec3::from_array(*b), factor)
                .to_array()
        },
    )?;

    Ok(NodeTransforms {
        translation: translation.to_array(),
        rotation: rotation.to_array(),
        scale: scale.to_array(),
        matrix,
        translation_track,
        rotation_track,
        scale_track,
    })
}

fn offset_matrix(node: &Rsm2Node) -> anyhow::Result<Mat3> {
    let matrix = Mat3::from_cols_array(&node.offset_matrix);
    ensure!(
        matrix.is_finite(),
        "node '{}' has a non-finite offset",
        node.name
    );
    ensure!(
        matrix.determinant().abs() > f32::EPSILON,
        "node '{}' has a singular offset",
        node.name
    );
    Ok(matrix)
}

fn decompose_basis(matrix: Mat3) -> anyhow::Result<(Vec3, Quat)> {
    ensure!(matrix.is_finite(), "non-finite matrix");
    let mut columns = [matrix.x_axis, matrix.y_axis, matrix.z_axis];
    let mut scale = Vec3::new(
        columns[0].length(),
        columns[1].length(),
        columns[2].length(),
    );
    ensure!(scale.min_element() > f32::EPSILON, "singular matrix");
    for index in 0..3 {
        columns[index] /= scale[index];
    }
    if Mat3::from_cols(columns[0], columns[1], columns[2]).determinant() < 0.0 {
        columns[0] = -columns[0];
        scale.x = -scale.x;
    }
    let rotation_matrix = Mat3::from_cols(columns[0], columns[1], columns[2]);
    ensure!(
        rotation_matrix
            .transpose()
            .mul_mat3(&rotation_matrix)
            .abs_diff_eq(Mat3::IDENTITY, MATRIX_EPSILON),
        "matrix contains shear"
    );
    let rotation = Quat::from_mat3(&rotation_matrix).normalize();
    let reconstructed = Mat3::from_quat(rotation) * Mat3::from_diagonal(scale);
    ensure!(
        reconstructed.abs_diff_eq(matrix, MATRIX_EPSILON),
        "matrix does not reconstruct from TRS"
    );
    Ok((scale, rotation))
}

fn boundary_track<T: Clone>(
    node: &str,
    property: &str,
    duration_ms: f32,
    base: T,
    source: impl IntoIterator<Item = anyhow::Result<NormalizedKey<T>>>,
    interpolate: impl Fn(&T, &T, f32) -> T,
) -> anyhow::Result<NormalizedTrack<T>> {
    let source = source.into_iter().collect::<anyhow::Result<Vec<_>>>()?;
    if source.is_empty() {
        return Ok(NormalizedTrack::default());
    }
    ensure!(
        duration_ms > 0.0,
        "node '{node}' has {property} keys with zero duration"
    );
    ensure!(
        !source
            .windows(2)
            .any(|pair| pair[1].time_ms <= pair[0].time_ms),
        "node '{node}' has collapsed/non-increasing {property} key times"
    );

    let mut keys = source
        .iter()
        .filter(|key| key.time_ms >= 0.0 && key.time_ms <= duration_ms)
        .cloned()
        .collect::<Vec<_>>();
    if keys.first().is_none_or(|key| key.time_ms > 0.0) {
        keys.insert(
            0,
            NormalizedKey {
                time_ms: 0.0,
                value: sample_curve(&source, 0.0, &base, &interpolate),
            },
        );
    }
    if keys.last().is_some_and(|key| key.time_ms < duration_ms) {
        keys.push(NormalizedKey {
            time_ms: duration_ms,
            value: sample_curve(&source, duration_ms, &base, &interpolate),
        });
    }
    Ok(NormalizedTrack { keys })
}

fn sample_curve<T: Clone>(
    source: &[NormalizedKey<T>],
    time_ms: f32,
    base: &T,
    interpolate: &impl Fn(&T, &T, f32) -> T,
) -> T {
    let next = source.partition_point(|key| time_ms >= key.time_ms);
    if next == source.len() {
        return source.last().expect("non-empty source curve").value.clone();
    }
    let (previous_time, previous_value) = if next == 0 {
        (0.0, base)
    } else {
        (source[next - 1].time_ms, &source[next - 1].value)
    };
    let next_key = &source[next];
    if next_key.time_ms == previous_time {
        return next_key.value.clone();
    }
    let factor = (time_ms - previous_time) / (next_key.time_ms - previous_time);
    interpolate(previous_value, &next_key.value, factor)
}

fn source_key<T>(raw_time: i32, fps: f32, value: T) -> anyhow::Result<NormalizedKey<T>> {
    Ok(NormalizedKey {
        time_ms: source_time(raw_time, fps)?,
        value,
    })
}

fn source_time(raw_time: i32, fps: f32) -> anyhow::Result<f32> {
    let time = (raw_time as f32 * fps).ceil();
    ensure!(time.is_finite(), "invalid RSM2 time");
    Ok(time)
}

fn exact_u32(value: f32, field: &str) -> anyhow::Result<u32> {
    ensure!(
        value >= 0.0 && value <= u32::MAX as f32,
        "{field} exceeds u32"
    );
    Ok(value as u32)
}

fn texture_animations(
    node: &Rsm2Node,
    duration_ms: u32,
    fps: f32,
) -> anyhow::Result<BTreeMap<usize, LifUvAnimation>> {
    let mut animations = BTreeMap::new();
    for animation in &node.texture_animations {
        let normalized = normalize_uv_animation(animation, duration_ms, fps)
            .with_context(|| format!("normalizing texture animation in node '{}'", node.name))?;
        ensure!(
            animations
                .insert(animation.texture_index, normalized)
                .is_none(),
            "node '{}' has duplicate animation for texture slot {}",
            node.name,
            animation.texture_index
        );
    }
    Ok(animations)
}

fn normalize_uv_animation(
    animation: &Rsm2TextureAnimation,
    duration_ms: u32,
    fps: f32,
) -> anyhow::Result<LifUvAnimation> {
    let channels = animation
        .channels
        .iter()
        .map(|channel| {
            let property = match channel.channel_type {
                Rsm2TextureChannelType::TranslateU => LifUvProperty::TranslateU,
                Rsm2TextureChannelType::TranslateV => LifUvProperty::TranslateV,
                Rsm2TextureChannelType::ScaleU => LifUvProperty::ScaleU,
                Rsm2TextureChannelType::ScaleV => LifUvProperty::ScaleV,
                Rsm2TextureChannelType::Rotate => LifUvProperty::Rotate,
            };
            let source = channel
                .keys
                .iter()
                .map(|key| {
                    Ok(NormalizedKey {
                        time_ms: source_time(key.time, fps)?,
                        value: key.value,
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            ensure!(
                !source
                    .windows(2)
                    .any(|pair| pair[1].time_ms <= pair[0].time_ms),
                "collapsed/non-increasing UV key times"
            );
            let had_negative = source.iter().any(|key| key.time_ms < 0.0);
            let has_late = source.iter().any(|key| key.time_ms > duration_ms as f32);
            let mut keys = source
                .iter()
                .filter(|key| key.time_ms >= 0.0 && key.time_ms <= duration_ms as f32)
                .map(|key| {
                    Ok(LifScalarKey {
                        time_ms: exact_u32(key.time_ms, "UV key time")?,
                        value: key.value,
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            if had_negative && keys.first().is_none_or(|key| key.time_ms > 0) {
                keys.insert(
                    0,
                    LifScalarKey {
                        time_ms: 0,
                        value: sample_curve(&source, 0.0, &0.0, &|left, right, factor| {
                            left + factor * (right - left)
                        }),
                    },
                );
            }
            if has_late && keys.last().is_none_or(|key| key.time_ms < duration_ms) {
                keys.push(LifScalarKey {
                    time_ms: duration_ms,
                    value: sample_curve(
                        &source,
                        duration_ms as f32,
                        &0.0,
                        &|left, right, factor| left + factor * (right - left),
                    ),
                });
            }
            Ok(LifUvChannel { property, keys })
        })
        .collect::<anyhow::Result<_>>()?;
    let animation = LifUvAnimation {
        duration_ms,
        channels,
    };
    animation.validate().map_err(anyhow::Error::msg)?;
    Ok(animation)
}

fn geometry(
    node: &Rsm2Node,
    local_textures: &[usize],
    textures: &[String],
    animations: &BTreeMap<usize, LifUvAnimation>,
    shading: ShadingPolicy,
) -> anyhow::Result<Vec<RawPrimitive>> {
    let face_normals: Vec<Vec3> = node
        .faces
        .iter()
        .map(|face| {
            let [a, b, c] = face
                .vertex_indices
                .map(|index| Vec3::from_array(node.vertices[index]));
            (b - a).cross(c - a).normalize_or_zero()
        })
        .collect();
    let mut smooth = HashMap::<(i32, usize), Vec3>::new();
    if shading == ShadingPolicy::Smooth {
        for (face, normal) in node.faces.iter().zip(&face_normals) {
            for group in &face.smooth_groups {
                for vertex in face.vertex_indices {
                    *smooth.entry((*group, vertex)).or_default() += *normal;
                }
            }
        }
    }

    let mut groups = BTreeMap::<PrimitiveKey, Vec<usize>>::new();
    for (face_index, face) in node.faces.iter().enumerate() {
        let texture = local_textures[face.texture_index];
        let animated = animations.contains_key(&face.texture_index);
        let material = MaterialKey {
            texture,
            two_sided: face.two_sided > 0 || (animated && is_tga(&textures[texture])),
        };
        groups
            .entry(PrimitiveKey {
                material,
                uv_target: animated.then_some(face.texture_index),
            })
            .or_default()
            .push(face_index);
    }

    groups
        .into_iter()
        .map(|(key, face_indices)| {
            let mut primitive = RawPrimitive {
                key,
                positions: Vec::new(),
                normals: Vec::new(),
                uvs: Vec::new(),
                indices: Vec::new(),
            };
            let mut corners = HashMap::<CornerKey, u32>::new();
            for face_index in face_indices {
                let face = &node.faces[face_index];
                for source_corner in [0, 2, 1] {
                    let position_index = face.vertex_indices[source_corner];
                    let uv_index = face.texture_vertex_indices[source_corner];
                    let (normal, normal_identity) = match shading {
                        ShadingPolicy::Smooth => {
                            let group = face.smooth_groups[0];
                            (
                                smooth
                                    .get(&(group, position_index))
                                    .copied()
                                    .unwrap_or(Vec3::ZERO)
                                    .normalize_or_zero(),
                                i64::from(group),
                            )
                        }
                        ShadingPolicy::None | ShadingPolicy::Flat => {
                            (face_normals[face_index], -(face_index as i64) - 1)
                        }
                    };
                    let corner = CornerKey {
                        position: position_index,
                        uv: uv_index,
                        normal_identity,
                    };
                    let index = if let Some(index) = corners.get(&corner) {
                        *index
                    } else {
                        let index = primitive.positions.len() as u32;
                        primitive.positions.push(
                            (SOURCE_BASIS * Vec3::from_array(node.vertices[position_index]))
                                .to_array(),
                        );
                        primitive
                            .normals
                            .push((SOURCE_BASIS * normal).normalize_or_zero().to_array());
                        primitive
                            .uvs
                            .push(node.texture_vertices[uv_index].coordinates);
                        corners.insert(corner, index);
                        index
                    };
                    primitive.indices.push(index);
                }
            }
            ensure!(
                primitive
                    .positions
                    .iter()
                    .flatten()
                    .chain(primitive.normals.iter().flatten())
                    .all(|value| value.is_finite()),
                "node '{}' produced non-finite geometry",
                node.name
            );
            Ok(primitive)
        })
        .collect()
}

fn alpha_mode(texture: &str) -> AlphaMode {
    if is_tga(texture) {
        AlphaMode::Blend
    } else {
        AlphaMode::Mask { cutoff: 0.8 }
    }
}

fn is_tga(texture: &str) -> bool {
    texture
        .rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("tga"))
}

fn transform_uv(uv: [f32; 2], sample: LifUvSample) -> [f32; 2] {
    let matrix = sample.matrix3();
    let uv = Vec2::from_array(uv);
    [
        matrix[0] * uv.x + matrix[1] * uv.y + matrix[2],
        matrix[3] * uv.x + matrix[4] * uv.y + matrix[5],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ro_formats::rsm2::{
        Rsm2Face, Rsm2PositionKey, Rsm2RotationKey, Rsm2ScaleKey, Rsm2TextureChannel,
        Rsm2TextureKey, Rsm2TextureVertex,
    };

    fn face(texture_index: usize, two_sided: i32, smooth_group: i32) -> Rsm2Face {
        Rsm2Face {
            record_length: 24,
            vertex_indices: [0, 1, 2],
            texture_vertex_indices: [0, 1, 2],
            texture_index,
            padding: 0,
            two_sided,
            smooth_groups: vec![smooth_group],
            unknown_words: Vec::new(),
        }
    }

    fn node(name: &str, parent: &str, textures: Rsm2NodeTextures) -> Rsm2Node {
        Rsm2Node {
            name: name.into(),
            parent_name: parent.into(),
            textures,
            offset_matrix: Mat3::IDENTITY.to_cols_array(),
            offset_position: [0.0; 3],
            vertices: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            texture_vertices: vec![
                Rsm2TextureVertex {
                    unknown: 0.0,
                    coordinates: [0.0, 0.0],
                },
                Rsm2TextureVertex {
                    unknown: 0.0,
                    coordinates: [1.0, 0.0],
                },
                Rsm2TextureVertex {
                    unknown: 0.0,
                    coordinates: [0.0, 1.0],
                },
            ],
            faces: vec![face(0, 0, 7)],
            scale_keys: Vec::new(),
            rotation_keys: Vec::new(),
            position_keys: Vec::new(),
            texture_animations: Vec::new(),
        }
    }

    fn model(version: Rsm2Version, nodes: Vec<Rsm2Node>, roots: &[&str]) -> Rsm2 {
        Rsm2 {
            version,
            animation_length: 10,
            shade_type: 2,
            alpha: 1,
            frames_per_second: 2.5,
            global_textures: if version == Rsm2Version::V2_2 {
                vec!["a.bmp".into(), "b.tga".into()]
            } else {
                Vec::new()
            },
            roots: roots.iter().map(|root| (*root).into()).collect(),
            nodes,
        }
    }

    #[test]
    fn resolves_both_texture_layouts_and_preserves_roots_and_parents() {
        let mut root = node("root", "", Rsm2NodeTextures::GlobalIndices(vec![1]));
        root.faces[0].two_sided = 1;
        let other = node("other", "", Rsm2NodeTextures::GlobalIndices(vec![0]));
        let child = node("child", "root", Rsm2NodeTextures::GlobalIndices(vec![0]));
        let build = build_model(
            &model(
                Rsm2Version::V2_2,
                vec![root, child, other],
                &["root", "other"],
            ),
            "h",
        )
        .unwrap();
        assert_eq!(build.textures, ["a.bmp", "b.tga"]);
        assert_eq!(build.roots, [0, 2]);
        assert_eq!(build.nodes[1].parent, Some(0));
        assert!(matches!(
            build.materials[0].alpha,
            AlphaMode::Mask { cutoff: 0.8 }
        ));
        assert!(
            build
                .materials
                .iter()
                .any(|material| matches!(material.alpha, AlphaMode::Blend))
        );

        let first = node(
            "first",
            "",
            Rsm2NodeTextures::Names(vec!["z.tga".into(), "a.bmp".into()]),
        );
        let mut second = node("second", "", Rsm2NodeTextures::Names(vec!["a.bmp".into()]));
        second.faces[0].texture_index = 0;
        let build = build_model(
            &model(Rsm2Version::V2_3, vec![first, second], &["first", "second"]),
            "h",
        )
        .unwrap();
        assert_eq!(build.textures, ["z.tga", "a.bmp"]);
    }

    #[test]
    fn converts_frame_like_times_and_pins_base_and_terminal_values() {
        let mut source = node("root", "", Rsm2NodeTextures::GlobalIndices(vec![0]));
        source.offset_position = [8.0, 2.0, 4.0];
        source.position_keys = vec![Rsm2PositionKey {
            time: 1,
            position: [2.0, 3.0, 4.0],
            unknown: 0.0,
        }];
        source.rotation_keys = vec![Rsm2RotationKey {
            time: 2,
            quaternion_xyzw: Quat::IDENTITY.to_array(),
        }];
        source.scale_keys = vec![Rsm2ScaleKey {
            time: 3,
            scale: [2.0; 3],
            unknown: 0.0,
        }];
        let build = build_model(&model(Rsm2Version::V2_2, vec![source], &["root"]), "h").unwrap();
        assert_eq!(build.duration_ms, 25.0);
        assert_eq!(build.nodes[0].translation, [8.0, -2.0, 4.0]);
        assert_eq!(
            build.nodes[0]
                .translation_track
                .keys
                .iter()
                .map(|key| key.time_ms)
                .collect::<Vec<_>>(),
            [0.0, 3.0, 25.0]
        );
        assert_eq!(
            build.nodes[0]
                .rotation_track
                .keys
                .iter()
                .map(|key| key.time_ms)
                .collect::<Vec<_>>(),
            [0.0, 5.0, 25.0]
        );
        assert_eq!(
            build.nodes[0]
                .scale_track
                .keys
                .iter()
                .map(|key| key.time_ms)
                .collect::<Vec<_>>(),
            [0.0, 8.0, 25.0]
        );
        assert_eq!(
            build.nodes[0].translation_track.keys.last().unwrap().value,
            [2.0, -3.0, 4.0]
        );
    }

    #[test]
    fn unkeyed_hierarchy_matches_source_matrix_and_map_placement() {
        use glam::Mat4;

        let root_offset = Mat3::from_diagonal(Vec3::new(2.0, 3.0, 1.0));
        let child_local = Mat3::from_diagonal(Vec3::new(0.5, 2.0, 1.0));
        let root_position = Vec3::new(10.0, 20.0, 30.0);
        let child_delta = Vec3::new(1.0, 2.0, 3.0);

        let mut root = node("root", "", Rsm2NodeTextures::GlobalIndices(vec![0]));
        root.offset_matrix = root_offset.to_cols_array();
        root.offset_position = root_position.to_array();
        let mut child = node("child", "root", Rsm2NodeTextures::GlobalIndices(vec![0]));
        child.offset_matrix = (root_offset * child_local).to_cols_array();
        child.offset_position = (root_position + root_offset * child_delta).to_array();

        let build =
            build_model(&model(Rsm2Version::V2_2, vec![root, child], &["root"]), "h").unwrap();
        let matrix = |node: &NormalizedNode| {
            Mat4::from_scale_rotation_translation(
                Vec3::from_array(node.scale),
                Quat::from_array(node.rotation),
                Vec3::from_array(node.translation),
            )
        };
        let actual = matrix(&build.nodes[0]) * matrix(&build.nodes[1]);
        let basis = Mat4::from_mat3(SOURCE_BASIS);
        let source_world = Mat4::from_translation(root_position)
            * Mat4::from_mat3(root_offset)
            * Mat4::from_translation(child_delta)
            * Mat4::from_mat3(child_local);
        let expected = basis * source_world * basis;
        let placement = Mat4::from_scale_rotation_translation(
            Vec3::new(1.5, 0.75, 2.0),
            Quat::from_rotation_z(0.3),
            Vec3::new(4.0, 5.0, 6.0),
        );
        let point = Vec3::new(0.25, 0.5, 0.75).extend(1.0);
        assert!((placement * actual * point).abs_diff_eq(placement * expected * point, 1.0e-4));
    }

    #[test]
    fn keyed_interpolation_and_terminal_hold_match_source_matrix() {
        use glam::Mat4;

        let mut source_node = node("root", "", Rsm2NodeTextures::GlobalIndices(vec![0]));
        source_node.offset_position = [2.0, 4.0, 6.0];
        let source_rotation = Quat::from_rotation_z(0.8);
        source_node.position_keys.push(Rsm2PositionKey {
            time: 2,
            position: [6.0, 8.0, 10.0],
            unknown: 0.0,
        });
        source_node.rotation_keys.push(Rsm2RotationKey {
            time: 2,
            quaternion_xyzw: source_rotation.to_array(),
        });
        source_node.scale_keys.push(Rsm2ScaleKey {
            time: 2,
            scale: [3.0, 5.0, 7.0],
            unknown: 0.0,
        });
        let build =
            build_model(&model(Rsm2Version::V2_2, vec![source_node], &["root"]), "h").unwrap();
        let node = &build.nodes[0];
        let halfway = 0.5;
        let translation = Vec3::from_array(node.translation_track.keys[0].value).lerp(
            Vec3::from_array(node.translation_track.keys[1].value),
            halfway,
        );
        let rotation = Quat::from_array(node.rotation_track.keys[0].value)
            .slerp(Quat::from_array(node.rotation_track.keys[1].value), halfway);
        let scale = Vec3::from_array(node.scale_track.keys[0].value)
            .lerp(Vec3::from_array(node.scale_track.keys[1].value), halfway);
        let actual = Mat4::from_scale_rotation_translation(scale, rotation, translation);

        let source_translation = Vec3::new(2.0, 4.0, 6.0).lerp(Vec3::new(6.0, 8.0, 10.0), halfway);
        let source_rotation = Quat::IDENTITY.slerp(source_rotation, halfway);
        let source_scale = Vec3::ONE.lerp(Vec3::new(3.0, 5.0, 7.0), halfway);
        let basis = Mat4::from_mat3(SOURCE_BASIS);
        let expected = basis
            * Mat4::from_scale_rotation_translation(
                source_scale,
                source_rotation,
                source_translation,
            )
            * basis;
        assert!(actual.abs_diff_eq(expected, 1.0e-4));
        assert_eq!(node.translation_track.keys.last().unwrap().time_ms, 25.0);
        assert_eq!(
            node.translation_track.keys.last().unwrap().value,
            [6.0, -8.0, 10.0]
        );
    }

    #[test]
    fn shear_helpers_preserve_animated_world_matrices_and_descendants() {
        let shear = Mat3::from_cols(Vec3::X, Vec3::new(0.5, 1.0, 0.0), Vec3::Z);
        let child_basis = Mat3::from_quat(Quat::from_rotation_z(0.3))
            * Mat3::from_diagonal(Vec3::new(1.5, 0.75, 2.0));
        let root_position = Vec3::new(2.0, 4.0, 6.0);
        let child_delta = Vec3::new(1.0, 2.0, 3.0);
        let mut root = node("root", "", Rsm2NodeTextures::GlobalIndices(vec![0]));
        root.offset_matrix = shear.to_cols_array();
        root.offset_position = root_position.to_array();
        root.position_keys = [root_position, Vec3::new(8.0, 10.0, 12.0)]
            .into_iter()
            .enumerate()
            .map(|(index, position)| Rsm2PositionKey {
                time: index as i32 * 10,
                position: position.to_array(),
                unknown: 0.0,
            })
            .collect();
        root.scale_keys = [Vec3::ONE, Vec3::new(2.0, 3.0, 4.0)]
            .into_iter()
            .enumerate()
            .map(|(index, scale)| Rsm2ScaleKey {
                time: index as i32 * 10,
                scale: scale.to_array(),
                unknown: 0.0,
            })
            .collect();
        let mut child = node("child", "root", Rsm2NodeTextures::GlobalIndices(vec![0]));
        child.offset_matrix = (shear * child_basis).to_cols_array();
        child.offset_position = (root_position + shear * child_delta).to_array();

        let build =
            build_model(&model(Rsm2Version::V2_2, vec![root, child], &["root"]), "h").unwrap();
        assert_eq!(build.nodes.len(), 4);
        assert_eq!(build.nodes[1].name, "__rsm2_matrix_0");
        assert_eq!(build.nodes[2].name, "__rsm2_scale_0");
        assert_eq!(build.nodes[3].parent, Some(2));
        assert!(build.nodes[0].primitives.is_empty());
        assert!(build.nodes[1].primitives.is_empty());
        assert!(!build.nodes[2].primitives.is_empty());

        let basis = Mat4::from_mat3(SOURCE_BASIS);
        for time in [0.0, 12.5, 25.0] {
            let factor = time / 25.0;
            let translation = root_position.lerp(Vec3::new(8.0, 10.0, 12.0), factor);
            let scale = Vec3::ONE.lerp(Vec3::new(2.0, 3.0, 4.0), factor);
            let actual_translation =
                Vec3::from_array(build.nodes[0].translation_track.keys[0].value).lerp(
                    Vec3::from_array(build.nodes[0].translation_track.keys[1].value),
                    factor,
                );
            let actual_scale = Vec3::from_array(build.nodes[2].scale_track.keys[0].value).lerp(
                Vec3::from_array(build.nodes[2].scale_track.keys[1].value),
                factor,
            );
            let actual = Mat4::from_translation(actual_translation)
                * Mat4::from_cols_array(&build.nodes[1].matrix.unwrap())
                * Mat4::from_scale(actual_scale)
                * Mat4::from_scale_rotation_translation(
                    Vec3::from_array(build.nodes[3].scale),
                    Quat::from_array(build.nodes[3].rotation),
                    Vec3::from_array(build.nodes[3].translation),
                );
            let expected = basis
                * Mat4::from_translation(translation)
                * Mat4::from_mat3(shear)
                * Mat4::from_scale(scale)
                * Mat4::from_translation(child_delta)
                * Mat4::from_mat3(child_basis)
                * basis;
            assert!(actual.abs_diff_eq(expected, MATRIX_EPSILON));
        }
    }

    #[test]
    fn late_translation_key_is_sampled_at_the_declared_loop_boundary() {
        let mut source = node("root", "", Rsm2NodeTextures::GlobalIndices(vec![0]));
        source.position_keys = vec![
            Rsm2PositionKey {
                time: 2,
                position: [2.0, 4.0, 6.0],
                unknown: 0.0,
            },
            Rsm2PositionKey {
                time: 20,
                position: [12.0, 14.0, 16.0],
                unknown: 0.0,
            },
        ];
        let build = build_model(&model(Rsm2Version::V2_2, vec![source], &["root"]), "h").unwrap();
        let keys = &build.nodes[0].translation_track.keys;
        assert_eq!(
            keys.iter().map(|key| key.time_ms).collect::<Vec<_>>(),
            [0.0, 5.0, 25.0]
        );
        let expected = Vec3::new(2.0, 4.0, 6.0).lerp(Vec3::new(12.0, 14.0, 16.0), 20.0 / 45.0);
        assert!(Vec3::from_array(keys[2].value).abs_diff_eq(SOURCE_BASIS * expected, 1.0e-5));
    }

    #[test]
    fn late_uv_key_is_sampled_at_the_declared_loop_boundary() {
        let mut source = node("root", "", Rsm2NodeTextures::GlobalIndices(vec![0]));
        source.texture_animations = vec![Rsm2TextureAnimation {
            texture_index: 0,
            channels: vec![Rsm2TextureChannel {
                channel_type: Rsm2TextureChannelType::TranslateU,
                keys: vec![
                    Rsm2TextureKey {
                        time: 2,
                        value: 2.0,
                    },
                    Rsm2TextureKey {
                        time: 20,
                        value: 12.0,
                    },
                ],
            }],
        }];
        let build = build_model(&model(Rsm2Version::V2_2, vec![source], &["root"]), "h").unwrap();
        let keys = &build.nodes[0].primitives[0]
            .uv_animation
            .as_ref()
            .unwrap()
            .channels[0]
            .keys;
        assert_eq!(
            keys.iter().map(|key| key.time_ms).collect::<Vec<_>>(),
            [5, 25]
        );
        assert!((keys[1].value - (2.0 + 20.0 / 45.0 * 10.0)).abs() < 1.0e-5);
    }

    #[test]
    fn negative_keys_are_sampled_at_time_zero() {
        let mut source = node("root", "", Rsm2NodeTextures::GlobalIndices(vec![0]));
        source.position_keys = vec![
            Rsm2PositionKey {
                time: -4,
                position: [2.0, 2.0, 2.0],
                unknown: 0.0,
            },
            Rsm2PositionKey {
                time: 2,
                position: [12.0, 12.0, 12.0],
                unknown: 0.0,
            },
        ];
        source.texture_animations = vec![Rsm2TextureAnimation {
            texture_index: 0,
            channels: vec![Rsm2TextureChannel {
                channel_type: Rsm2TextureChannelType::TranslateU,
                keys: vec![
                    Rsm2TextureKey {
                        time: -4,
                        value: 2.0,
                    },
                    Rsm2TextureKey {
                        time: 2,
                        value: 12.0,
                    },
                ],
            }],
        }];

        let build = build_model(&model(Rsm2Version::V2_2, vec![source], &["root"]), "h").unwrap();
        let expected = 2.0 + 10.0 / 15.0 * 10.0;
        let translation = &build.nodes[0].translation_track.keys;
        assert_eq!(translation[0].time_ms, 0.0);
        assert!(
            Vec3::from_array(translation[0].value)
                .abs_diff_eq(SOURCE_BASIS * Vec3::splat(expected), 1.0e-5)
        );
        let uv = &build.nodes[0].primitives[0]
            .uv_animation
            .as_ref()
            .unwrap()
            .channels[0]
            .keys;
        assert_eq!(uv[0].time_ms, 0);
        assert!((uv[0].value - expected).abs() < 1.0e-5);
    }

    #[test]
    fn decomposes_basis_conjugated_offsets_and_rejects_shear() {
        let rotation = Quat::from_rotation_z(0.4);
        let source_matrix =
            Mat3::from_quat(rotation) * Mat3::from_diagonal(Vec3::new(2.0, 3.0, 4.0));
        let conjugated = SOURCE_BASIS * source_matrix * SOURCE_BASIS;
        let (scale, rotation) = decompose_basis(conjugated).unwrap();
        assert!(
            (Mat3::from_quat(rotation) * Mat3::from_diagonal(scale))
                .abs_diff_eq(conjugated, MATRIX_EPSILON)
        );

        let shear = Mat3::from_cols(Vec3::X, Vec3::new(0.5, 1.0, 0.0), Vec3::Z);
        assert!(
            decompose_basis(shear)
                .unwrap_err()
                .to_string()
                .contains("shear")
        );
    }

    #[test]
    fn keyed_rotation_ignores_an_unsupported_static_offset() {
        let mut source = node("root", "", Rsm2NodeTextures::GlobalIndices(vec![0]));
        source.offset_matrix =
            Mat3::from_cols(Vec3::X, Vec3::new(0.5, 1.0, 0.0), Vec3::Z).to_cols_array();
        source.rotation_keys.push(Rsm2RotationKey {
            time: 0,
            quaternion_xyzw: Quat::IDENTITY.to_array(),
        });

        let build = build_model(&model(Rsm2Version::V2_2, vec![source], &["root"]), "h")
            .expect("keyed rotation ignores the source offset");

        assert_eq!(build.nodes[0].rotation, Quat::IDENTITY.to_array());
        assert_eq!(build.nodes[0].scale, Vec3::ONE.to_array());
    }

    #[test]
    fn reverses_winding_reflects_normals_and_keeps_degenerate_faces_finite() {
        let mut source = node("root", "", Rsm2NodeTextures::GlobalIndices(vec![0]));
        source.faces.push(face(0, 0, 7));
        source.faces[1].vertex_indices = [0, 0, 0];
        let build = build_model(&model(Rsm2Version::V2_2, vec![source], &["root"]), "h").unwrap();
        let primitive = &build.nodes[0].primitives[0];
        assert_eq!(&primitive.indices[..3], &[0, 1, 2]);
        assert_eq!(primitive.positions[1], [0.0, -1.0, 0.0]);
        assert!(
            primitive
                .normals
                .iter()
                .flatten()
                .all(|value| value.is_finite())
        );
    }

    #[test]
    fn preserves_uv_on_non_tga_and_forces_two_sided_only_for_animated_tga() {
        let mut source = node(
            "root",
            "",
            Rsm2NodeTextures::Names(vec!["a.bmp".into(), "b.tga".into()]),
        );
        source.faces = vec![face(0, 0, 1), face(1, 0, 1)];
        source.texture_animations = [0, 1]
            .into_iter()
            .map(|texture_index| Rsm2TextureAnimation {
                texture_index,
                channels: [
                    (Rsm2TextureChannelType::TranslateU, 0.25),
                    (Rsm2TextureChannelType::TranslateV, 0.5),
                    (Rsm2TextureChannelType::ScaleU, 1.5),
                    (Rsm2TextureChannelType::ScaleV, 0.75),
                    (Rsm2TextureChannelType::Rotate, 0.2),
                ]
                .into_iter()
                .map(|(channel_type, value)| Rsm2TextureChannel {
                    channel_type,
                    keys: vec![Rsm2TextureKey { time: 0, value }],
                })
                .collect(),
            })
            .collect();
        let build = build_model(&model(Rsm2Version::V2_3, vec![source], &["root"]), "h").unwrap();
        assert_eq!(build.nodes[0].primitives.len(), 2);
        let source_uvs = vec![[0.0, 0.0], [0.0, 1.0], [1.0, 0.0]];
        assert!(build.nodes[0].primitives.iter().all(|primitive| {
            primitive.uv1.as_ref() == Some(&source_uvs)
                && primitive.uv0 != source_uvs
                && primitive.uv_animation.as_ref().is_some_and(|animation| {
                    animation.channels.len() == 5
                        && animation
                            .channels
                            .iter()
                            .all(|channel| channel.keys.len() == 1 && channel.keys[0].time_ms == 0)
                })
        }));
        let bmp = build
            .materials
            .iter()
            .find(|material| material.texture == Some(0))
            .unwrap();
        let tga = build
            .materials
            .iter()
            .find(|material| material.texture == Some(1))
            .unwrap();
        assert!(!bmp.two_sided);
        assert!(tga.two_sided);
    }

    #[test]
    fn smooth_normals_average_adjacent_faces_and_flat_corners_split() {
        let build = |shade_type| {
            let mut source_node = node("root", "", Rsm2NodeTextures::GlobalIndices(vec![0]));
            source_node.vertices.push([0.0, 0.0, 1.0]);
            source_node.texture_vertices.push(Rsm2TextureVertex {
                unknown: 0.0,
                coordinates: [1.0, 1.0],
            });
            let mut adjacent = face(0, 0, 7);
            adjacent.vertex_indices = [0, 3, 1];
            adjacent.texture_vertex_indices = [0, 3, 1];
            source_node.faces.push(adjacent);
            let mut source = model(Rsm2Version::V2_2, vec![source_node], &["root"]);
            source.shade_type = shade_type;
            build_model(&source, "h").unwrap()
        };

        let smooth = build(2);
        let smooth_primitive = &smooth.nodes[0].primitives[0];
        assert_eq!(smooth_primitive.positions.len(), 4);
        let shared = smooth_primitive
            .positions
            .iter()
            .position(|position| *position == [0.0; 3])
            .unwrap();
        assert!(
            Vec3::from_array(smooth_primitive.normals[shared])
                .abs_diff_eq(Vec3::new(0.0, -1.0, 1.0).normalize(), 1.0e-5)
        );

        let flat = build(1);
        assert_eq!(flat.nodes[0].primitives[0].positions.len(), 6);
    }

    #[test]
    fn preserves_flat_smooth_and_no_shade_policy_with_full_corner_identity() {
        for (shade, expected) in [
            (0, ShadingPolicy::None),
            (1, ShadingPolicy::Flat),
            (2, ShadingPolicy::Smooth),
        ] {
            let mut source = model(
                Rsm2Version::V2_2,
                vec![node("root", "", Rsm2NodeTextures::GlobalIndices(vec![0]))],
                &["root"],
            );
            source.shade_type = shade;
            source.alpha = 0;
            let build = build_model(&source, "h").unwrap();
            assert_eq!(build.materials[0].shading, expected);
            assert!(matches!(
                build.materials[0].alpha,
                AlphaMode::Mask { cutoff: 0.8 }
            ));
        }
    }
}
