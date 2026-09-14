//! Re-read generated glTF and compare its skinned poses with the source sampler.

use super::{
    animation::{sample_local_pose, transform_matrix},
    writer::source_to_gltf,
};
use anyhow::{Context, ensure};
use glam::{Mat4, Quat, Vec3};
use ro_formats::gr2::Gr2File;

pub(super) fn validate(
    bytes: &[u8],
    source: &Gr2File,
    clips: &[(&str, &Gr2File)],
) -> anyhow::Result<()> {
    let parsed = gltf::Gltf::from_slice(bytes)?;
    ensure!(
        parsed
            .buffers()
            .all(|b| matches!(b.source(), gltf::buffer::Source::Bin)),
        "external GLB buffer"
    );
    ensure!(
        parsed
            .images()
            .all(|i| matches!(i.source(), gltf::image::Source::View { .. })),
        "external GLB image"
    );
    let (document, buffers, images) = gltf::import_slice(bytes)?;
    ensure!(
        !images.is_empty() && document.scenes().count() == 1 && document.default_scene().is_some(),
        "incomplete model scene"
    );
    let skin = document.skins().next().context("missing skin")?;
    let joints: Vec<_> = skin.joints().map(|n| n.index()).collect();
    let inverse: Vec<_> = skin
        .reader(|b| Some(&buffers[b.index()].0))
        .read_inverse_bind_matrices()
        .context("missing inverse bind matrices")?
        .map(|m| Mat4::from_cols_array_2d(&m))
        .collect();
    ensure!(joints.len() == inverse.len(), "inverse bind count mismatch");
    let model = &source.models[0];
    let skeleton = &source.skeletons[model.skeleton_index.context("missing source skeleton")?];
    ensure!(skeleton.bones.len() == joints.len(), "joint count mismatch");
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for &mesh in &model.mesh_indices {
        for v in &source.vertex_datas[source.meshes[mesh]
            .vertex_data_index
            .context("missing source vertices")?]
        .vertices
        {
            let p = Vec3::from_array(v.position);
            min = min.min(p);
            max = max.max(p);
        }
    }
    let tolerance = (max - min).length() * 0.001;
    ensure!(
        tolerance.is_finite() && tolerance > 0.0,
        "invalid model bounds"
    );
    let base: Vec<_> = document
        .nodes()
        .map(|n| Mat4::from_cols_array_2d(&n.transform().matrix()))
        .collect();
    let mut parents = vec![None; base.len()];
    for node in document.nodes() {
        for child in node.children() {
            ensure!(
                parents[child.index()].replace(node.index()).is_none(),
                "node has multiple parents"
            );
            ensure!(
                node.index() < child.index(),
                "exported hierarchy is not parent-first"
            );
        }
    }
    ensure!(
        document.animations().count() == clips.len(),
        "animation count mismatch"
    );
    let mesh_pairs: Vec<_> = document
        .meshes()
        .zip(model.mesh_indices.iter().copied())
        .collect();
    ensure!(
        document.meshes().count() == model.mesh_indices.len(),
        "mesh count mismatch"
    );
    for &(name, clip) in clips {
        let animation = document
            .animations()
            .find(|a| a.name() == Some(name))
            .context("missing named animation")?;
        let channels = animation
            .channels()
            .map(|channel| {
                let reader = channel.reader(|b| Some(&buffers[b.index()].0));
                let times: Vec<_> = reader
                    .read_inputs()
                    .context("missing animation times")?
                    .collect();
                ensure!(
                    times.len() >= 2 && times.windows(2).all(|p| p[0] < p[1]),
                    "invalid animation time order"
                );
                let values = match reader.read_outputs().context("missing animation values")? {
                    gltf::animation::util::ReadOutputs::Translations(v) => {
                        Samples::Translation(v.map(Vec3::from_array).collect())
                    }
                    gltf::animation::util::ReadOutputs::Scales(v) => {
                        Samples::Scale(v.map(Vec3::from_array).collect())
                    }
                    gltf::animation::util::ReadOutputs::Rotations(v) => {
                        Samples::Rotation(v.into_f32().map(Quat::from_array).collect())
                    }
                    _ => anyhow::bail!("unexpected animation channel"),
                };
                let value_count = match &values {
                    Samples::Translation(v) | Samples::Scale(v) => v.len(),
                    Samples::Rotation(v) => v.len(),
                };
                ensure!(
                    value_count == times.len()
                        && times[0] == 0.0
                        && times.last() == Some(&clip.animations[0].duration),
                    "animation samples do not cover the source clip"
                );
                Ok((channel.target().node().index(), times, values))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let duration = clip.animations[0].duration;
        let mut worst = 0.0_f32;
        let check_rate = super::writer::SAMPLE_RATE * 2.0;
        for step in 0..=(duration * check_rate).ceil() as usize {
            let time = (step as f32 / check_rate).min(duration);
            let mut trs: Vec<_> = base
                .iter()
                .map(|m| m.to_scale_rotation_translation())
                .collect();
            for (node, times, samples) in &channels {
                let high = times
                    .partition_point(|&t| t <= time)
                    .clamp(1, times.len() - 1);
                let low = high - 1;
                let factor = ((time - times[low]) / (times[high] - times[low])).clamp(0.0, 1.0);
                match samples {
                    Samples::Translation(v) => trs[*node].2 = v[low].lerp(v[high], factor),
                    Samples::Scale(v) => trs[*node].0 = v[low].lerp(v[high], factor),
                    Samples::Rotation(v) => {
                        trs[*node].1 = v[low].slerp(v[high], factor).normalize()
                    }
                }
            }
            let mut worlds = Vec::with_capacity(base.len());
            for (index, (s, r, t)) in trs.into_iter().enumerate() {
                let local = Mat4::from_scale_rotation_translation(s, r, t);
                worlds.push(parents[index].map_or(local, |p| worlds[p] * local));
            }
            let actual_palette: Vec<_> = joints
                .iter()
                .zip(&inverse)
                .map(|(&joint, inverse)| worlds[joint] * *inverse)
                .collect();
            let pose = sample_local_pose(source, clip, 0, time)?;
            let mut source_worlds: Vec<Mat4> = Vec::new();
            let root = source_to_gltf() * transform_matrix(&model.initial_placement)?;
            for (bone, local) in skeleton.bones.iter().zip(pose) {
                source_worlds.push(if bone.parent_index < 0 {
                    root * local
                } else {
                    source_worlds[bone.parent_index as usize] * local
                });
            }
            let expected_palette: Vec<_> = source_worlds
                .iter()
                .zip(&skeleton.bones)
                .map(|(w, b)| *w * Mat4::from_cols_array(&b.inverse_world))
                .collect();
            for (mesh, source_index) in &mesh_pairs {
                let original = &source.meshes[*source_index];
                let data = &source.vertex_datas[original
                    .vertex_data_index
                    .context("missing source vertices")?];
                let bindings = original
                    .bone_bindings
                    .iter()
                    .map(|n| {
                        skeleton
                            .bones
                            .iter()
                            .position(|b| &b.name == n)
                            .context("unknown source bone")
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?;
                for primitive in mesh.primitives() {
                    let reader = primitive.reader(|b| Some(&buffers[b.index()].0));
                    let positions = reader.read_positions().context("missing positions")?;
                    let joints = reader.read_joints(0).context("missing joints")?.into_u16();
                    let weights = reader
                        .read_weights(0)
                        .context("missing weights")?
                        .into_f32();
                    ensure!(
                        positions.len() == data.vertices.len(),
                        "vertex count changed"
                    );
                    for (((position, joints), weights), original) in
                        positions.zip(joints).zip(weights).zip(&data.vertices)
                    {
                        ensure!(
                            weights.iter().all(|w| w.is_finite() && *w >= 0.0)
                                && (weights.iter().sum::<f32>() - 1.0).abs() < 1e-5,
                            "invalid output skin weights"
                        );
                        let actual: Vec3 = joints
                            .iter()
                            .zip(weights)
                            .map(|(&j, w)| {
                                actual_palette
                                    .get(j as usize)
                                    .context("invalid output joint")
                                    .map(|p| p.transform_point3(Vec3::from_array(position)) * w)
                            })
                            .collect::<anyhow::Result<Vec<_>>>()?
                            .into_iter()
                            .sum();
                        let expected = if data.has_bone_weights {
                            let total: f32 = original.bone_weights.iter().map(|&w| w as f32).sum();
                            ensure!(total > 0.0, "invalid source weights");
                            (0..4)
                                .filter(|&i| original.bone_weights[i] > 0)
                                .map(|i| {
                                    let joint = bindings[original.bone_indices[i] as usize];
                                    expected_palette[joint]
                                        .transform_point3(Vec3::from_array(original.position))
                                        * (original.bone_weights[i] as f32 / total)
                                })
                                .sum()
                        } else {
                            expected_palette[*bindings.first().context("missing rigid binding")?]
                                .transform_point3(Vec3::from_array(original.position))
                        };
                        let error = actual.distance(expected);
                        ensure!(
                            error.is_finite() && error <= tolerance,
                            "{name} at {time}s: pose error {error} exceeds {tolerance}"
                        );
                        worst = worst.max(error);
                    }
                }
            }
        }
        println!("  {name}: {duration:.3}s, max pose error {worst:.6} (limit {tolerance:.6})");
    }
    Ok(())
}

enum Samples {
    Translation(Vec<Vec3>),
    Scale(Vec<Vec3>),
    Rotation(Vec<Quat>),
}
