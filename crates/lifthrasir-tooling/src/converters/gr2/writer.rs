//! Standard glTF mesh, skin, embedded image, and sampled animation export.

use super::animation::{decompose, sample_local_pose, transform_matrix};
use crate::converters::gltf_out::{self, BinChunk, GeometryAttributes};
use anyhow::{Context, ensure};
use glam::{Mat4, Quat, Vec3};
use gltf_json as json;
use image::ImageEncoder;
use lifthrasir_data::gr2::EMBLEM_MATERIAL;
use ro_formats::gr2::Gr2File;
use serde_json::json;

/// Guardian attack curves need denser sampling than the source's nominal 30 Hz step.
pub(super) const SAMPLE_RATE: f32 = 240.0;

/// Keep source coordinates local to both mesh and skin; one scene root changes Z-up to Y-up.
pub(super) fn source_to_gltf() -> Mat4 {
    Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2)
}

pub(super) fn build(file: &Gr2File, clips: &[(&str, &Gr2File)]) -> anyhow::Result<Vec<u8>> {
    let model = file.models.first().context("no GR2 model")?;
    ensure!(
        file.models.len() == 1,
        "multiple models in one GR2 are not supported"
    );
    let skeleton = file
        .skeletons
        .get(model.skeleton_index.context("model has no skeleton")?)
        .context("invalid skeleton")?;
    ensure!(
        !skeleton.bones.is_empty() && skeleton.bones.len() <= u16::MAX as usize,
        "invalid skeleton size"
    );
    let mut root = json::Root::default();
    root.asset.generator = Some("Lifthrasir GR2 converter".into());
    let mut bin = BinChunk::default();
    let mut emblem_count = 0;
    for (index, texture) in file.textures.iter().enumerate() {
        let rgba = texture
            .to_rgba()
            .with_context(|| format!("texture {}", texture.from_file_name))?;
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png).write_image(
            &rgba,
            texture.width as u32,
            texture.height as u32,
            image::ExtendedColorType::Rgba8,
        )?;
        let view = bin.push_view(&png, None);
        root.images.push(serde_json::from_value(
            json!({"bufferView":view.value(), "mimeType":"image/png"}),
        )?);
        root.textures
            .push(serde_json::from_value(json!({"source":index}))?);
        let is_emblem = texture.from_file_name.to_lowercase().contains("emblem");
        emblem_count += usize::from(is_emblem);
        let name = if is_emblem {
            EMBLEM_MATERIAL.to_owned()
        } else {
            format!("texture_{index}")
        };
        root.materials.push(serde_json::from_value(json!({
            "name": name, "doubleSided":true, "alphaMode": if texture.has_alpha() { "BLEND" } else { "OPAQUE" },
            "pbrMetallicRoughness":{"baseColorTexture":{"index":index}, "metallicFactor":0, "roughnessFactor":1},
            "extensions":{"KHR_materials_unlit":{}}
        }))?);
    }
    ensure!(emblem_count <= 1, "ambiguous emblem texture slot");
    root.extensions_used.push("KHR_materials_unlit".into());

    let placement = transform_matrix(&model.initial_placement)?;
    root.nodes.push(serde_json::from_value(
        json!({"name":"gr2_axis", "matrix":source_to_gltf().to_cols_array(), "children":[1]}),
    )?);
    root.nodes.push(serde_json::from_value(
        json!({"name":"gr2_model", "matrix":placement.to_cols_array(), "children":[]}),
    )?);
    let joint_start = root.nodes.len();
    let mut names = std::collections::HashSet::new();
    for (index, bone) in skeleton.bones.iter().enumerate() {
        ensure!(
            names.insert(&bone.name),
            "duplicate joint name {}",
            bone.name
        );
        ensure!(
            bone.parent_index == -1 || (0..index as i32).contains(&bone.parent_index),
            "invalid joint parent"
        );
        let (s, r, t) = decompose(transform_matrix(&bone.transform)?)?;
        root.nodes.push(serde_json::from_value(json!({"name":bone.name,"translation":t.to_array(),"rotation":r.to_array(),"scale":s.to_array()}))?);
    }
    for (index, bone) in skeleton.bones.iter().enumerate() {
        let parent = if bone.parent_index < 0 {
            1
        } else {
            joint_start + bone.parent_index as usize
        };
        root.nodes[parent]
            .children
            .get_or_insert_default()
            .push(json::Index::new((joint_start + index) as u32));
    }
    let inverse: Vec<_> = skeleton
        .bones
        .iter()
        .flat_map(|bone| bone.inverse_world)
        .collect();
    ensure!(
        inverse.iter().all(|v| v.is_finite()),
        "nonfinite inverse bind matrix"
    );
    let inverse_accessor = floats(&mut root, &mut bin, &inverse, json::accessor::Type::Mat4);
    root.skins.push(serde_json::from_value(json!({
        "name":"gr2_skin", "inverseBindMatrices":inverse_accessor.value(),
        "joints":(joint_start..joint_start+skeleton.bones.len()).collect::<Vec<_>>()
    }))?);

    for &mesh_index in &model.mesh_indices {
        let mesh = file
            .meshes
            .get(mesh_index)
            .context("invalid model mesh reference")?;
        let vertices = &file
            .vertex_datas
            .get(mesh.vertex_data_index.context("mesh has no vertices")?)
            .context("invalid vertex data")?;
        let topology = file
            .tri_topologies
            .get(mesh.topology_index.context("mesh has no topology")?)
            .context("invalid topology")?;
        ensure!(
            !vertices.vertices.is_empty() && !topology.groups.is_empty(),
            "empty model mesh"
        );
        let binding: Vec<_> = mesh
            .bone_bindings
            .iter()
            .map(|name| {
                skeleton
                    .bones
                    .iter()
                    .position(|bone| &bone.name == name)
                    .map(|i| i as u16)
                    .with_context(|| format!("unknown bone binding {name}"))
            })
            .collect::<anyhow::Result<_>>()?;
        let mut joints = Vec::new();
        let mut weights = Vec::new();
        for vertex in &vertices.vertices {
            if !vertices.has_bone_weights {
                joints.extend([
                    *binding.first().context("rigid mesh has no bone binding")?,
                    0,
                    0,
                    0,
                ]);
                weights.extend([1.0, 0.0, 0.0, 0.0]);
                continue;
            }
            let sum: f32 = vertex.bone_weights.iter().map(|&w| w as f32).sum();
            ensure!(sum > 0.0, "zero-sum skin weights");
            for (&slot, &weight) in vertex.bone_indices.iter().zip(&vertex.bone_weights) {
                joints.push(if weight == 0 {
                    0
                } else {
                    *binding
                        .get(slot as usize)
                        .context("invalid vertex bone slot")?
                });
                weights.push(weight as f32 / sum);
            }
        }
        let joint_bytes: Vec<_> = joints.iter().flat_map(|i| i.to_le_bytes()).collect();
        let view = bin.push_view(&joint_bytes, Some(json::buffer::Target::ArrayBuffer));
        let joint_accessor = json::Index::push(
            &mut root.accessors,
            gltf_out::accessor(
                view,
                vertices.vertices.len(),
                json::accessor::ComponentType::U16,
                json::accessor::Type::Vec4,
            ),
        );
        let weight_accessor = floats(&mut root, &mut bin, &weights, json::accessor::Type::Vec4);
        let positions: Vec<_> = vertices
            .vertices
            .iter()
            .map(|v| Vec3::from_array(v.position))
            .collect();
        let normals: Vec<_> = vertices
            .vertices
            .iter()
            .map(|v| Vec3::from_array(v.normal))
            .collect();
        let uvs: Vec<_> = vertices.vertices.iter().map(|v| v.uv).collect();
        ensure!(
            positions.iter().chain(&normals).all(|p| p.is_finite())
                && uvs.iter().flatten().all(|v| v.is_finite()),
            "nonfinite vertex attributes"
        );
        let mut primitives = Vec::new();
        for group in &topology.groups {
            ensure!(
                group.tri_first >= 0 && group.tri_count > 0 && group.material_index >= 0,
                "invalid triangle group"
            );
            let material_index = if mesh.material_indices.is_empty() {
                ensure!(
                    group.material_index == 0,
                    "unassigned mesh has a nonzero material index"
                );
                if root.materials.len() == file.textures.len() {
                    root.materials.push(serde_json::from_value(json!({"name":"untextured", "doubleSided":true, "extensions":{"KHR_materials_unlit":{}}, "pbrMetallicRoughness":{"metallicFactor":0,"roughnessFactor":1}}))?);
                }
                file.textures.len()
            } else {
                let material = file
                    .materials
                    .get(
                        *mesh
                            .material_indices
                            .get(group.material_index as usize)
                            .context("invalid group material binding")?,
                    )
                    .context("invalid material")?;
                let texture = material.texture_index.context("material has no texture")?;
                ensure!(texture < file.textures.len(), "invalid material texture");
                texture
            };
            let first = group.tri_first as usize * 3;
            let end = first + group.tri_count as usize * 3;
            let indices = topology
                .indices
                .get(first..end)
                .context("triangle group outside index buffer")?;
            ensure!(
                indices.iter().all(|&i| (i as usize) < positions.len()),
                "triangle index outside vertex buffer"
            );
            let mut primitive = gltf_out::push_geometry_primitive(
                &mut root,
                &mut bin,
                &mesh.name,
                &GeometryAttributes {
                    positions: &positions,
                    normals: &normals,
                    colors: None,
                    uvs: &uvs,
                    uv1: None,
                    indices,
                },
                json::Index::new(material_index as u32),
            )?;
            primitive.attributes.insert(
                json::validation::Checked::Valid(json::mesh::Semantic::Joints(0)),
                joint_accessor,
            );
            primitive.attributes.insert(
                json::validation::Checked::Valid(json::mesh::Semantic::Weights(0)),
                weight_accessor,
            );
            primitives.push(primitive);
        }
        let index = root.meshes.len();
        root.meshes.push(serde_json::from_value(
            json!({"name":mesh.name,"primitives":primitives}),
        )?);
        let node = root.nodes.len();
        root.nodes.push(serde_json::from_value(
            json!({"name":format!("mesh_{index}"),"mesh":index,"skin":0}),
        )?);
        root.nodes[1]
            .children
            .get_or_insert_default()
            .push(json::Index::new(node as u32));
    }
    ensure!(
        !root.meshes.is_empty(),
        "GR2 model has no renderable meshes"
    );
    for &(name, clip_file) in clips {
        write_animation(&mut root, &mut bin, file, clip_file, name, joint_start)?;
    }
    root.scenes.push(serde_json::from_value(
        json!({"name":"gr2_scene", "nodes":[0]}),
    )?);
    root.scene = Some(json::Index::new(0));
    root.buffers.push(serde_json::from_value(
        json!({"byteLength":bin.data.len()}),
    )?);
    root.buffer_views = bin.views;
    Ok(gltf_out::glb_container(
        &serde_json::to_vec(&root)?,
        &bin.data,
    ))
}

fn floats(
    root: &mut json::Root,
    bin: &mut BinChunk,
    values: &[f32],
    kind: json::accessor::Type,
) -> json::Index<json::Accessor> {
    let view = bin.push_view(&gltf_out::f32_bytes(values.iter().copied()), None);
    let count = values.len() / kind.multiplicity();
    json::Index::push(
        &mut root.accessors,
        gltf_out::accessor(view, count, json::accessor::ComponentType::F32, kind),
    )
}

fn write_animation(
    root: &mut json::Root,
    bin: &mut BinChunk,
    model: &Gr2File,
    clip: &Gr2File,
    name: &str,
    joint_start: usize,
) -> anyhow::Result<()> {
    let duration = clip.animations.first().context("missing clip")?.duration;
    ensure!(
        duration.is_finite() && duration > 0.0 && duration <= 600.0,
        "invalid clip duration"
    );
    let frames = (duration * SAMPLE_RATE).ceil() as usize;
    let times: Vec<_> = (0..=frames)
        .map(|i| (i as f32 / SAMPLE_RATE).min(duration))
        .collect();
    let input = floats(root, bin, &times, json::accessor::Type::Scalar);
    root.accessors[input.value()].min = Some(json!([0.0]));
    root.accessors[input.value()].max = Some(json!([duration]));
    let poses = times
        .iter()
        .map(|&time| sample_local_pose(model, clip, 0, time))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let mut samplers = Vec::new();
    let mut channels = Vec::new();
    for joint in 0..poses[0].len() {
        let mut translation = Vec::new();
        let mut rotation = Vec::new();
        let mut scale = Vec::new();
        let mut previous = Quat::IDENTITY;
        for pose in &poses {
            let (s, mut r, t) = decompose(pose[joint])?;
            if previous.dot(r) < 0.0 {
                r = -r;
            }
            previous = r;
            translation.extend(t.to_array());
            rotation.extend(r.to_array());
            scale.extend(s.to_array());
        }
        for (path, values, kind) in [
            ("translation", translation, json::accessor::Type::Vec3),
            ("rotation", rotation, json::accessor::Type::Vec4),
            ("scale", scale, json::accessor::Type::Vec3),
        ] {
            let output = floats(root, bin, &values, kind);
            let sampler = samplers.len();
            samplers.push(
                json!({"input":input.value(),"output":output.value(),"interpolation":"LINEAR"}),
            );
            channels
                .push(json!({"sampler":sampler,"target":{"node":joint_start+joint,"path":path}}));
        }
    }
    root.animations.push(serde_json::from_value(
        json!({"name":name,"samplers":samplers,"channels":channels}),
    )?);
    Ok(())
}
