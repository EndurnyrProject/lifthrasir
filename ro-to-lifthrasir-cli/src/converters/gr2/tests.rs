use super::*;
use crate::{
    config::LoaderConfig,
    grf_vfs::{AssetRead, GrfVfs},
};
use ro_formats::gr2::{self, Gr2File};
use std::path::Path;

fn retail_vfs() -> GrfVfs {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut config = LoaderConfig::from_path(&root.join("assets/convert.toml")).unwrap();
    for entry in &mut config.assets.grf {
        let path = Path::new(&entry.path);
        if path.is_relative() {
            let direct = root.join(path);
            entry.path = if direct.exists() {
                direct
            } else {
                root.join("assets").join(path)
            }
            .to_string_lossy()
            .into_owned();
        }
    }
    GrfVfs::open(&config.grfs_by_priority()).unwrap()
}

fn posed_vertices(file: &gr2::Gr2File, clip: &Gr2File, time: f32) -> Vec<glam::Vec3> {
    use glam::{Mat4, Vec3};
    let model = &file.models[0];
    let skeleton = &file.skeletons[model.skeleton_index.unwrap()];
    let pose = animation::sample_local_pose(file, clip, 0, time).unwrap();
    assert_eq!(pose.len(), skeleton.bones.len(), "sample every flag joint");
    let mut worlds: Vec<Mat4> = Vec::new();
    for (bone, local) in skeleton.bones.iter().zip(pose) {
        worlds.push(if bone.parent_index < 0 {
            local
        } else {
            worlds[bone.parent_index as usize] * local
        });
    }
    let palette: Vec<_> = worlds
        .iter()
        .zip(&skeleton.bones)
        .map(|(world, bone)| *world * Mat4::from_cols_array(&bone.inverse_world))
        .collect();
    model
        .mesh_indices
        .iter()
        .flat_map(|&index| {
            let mesh = &file.meshes[index];
            let joints: Vec<_> = mesh
                .bone_bindings
                .iter()
                .map(|name| skeleton.bones.iter().position(|b| &b.name == name).unwrap())
                .collect();
            let data = &file.vertex_datas[mesh.vertex_data_index.unwrap()];
            data.vertices
                .iter()
                .map(|v| {
                    if !data.has_bone_weights {
                        return palette[joints[0]].transform_point3(Vec3::from_array(v.position));
                    }
                    let total: f32 = v.bone_weights.iter().map(|&w| w as f32).sum();
                    assert!(total > 0.0);
                    (0..4)
                        .filter(|&i| v.bone_weights[i] > 0)
                        .map(|i| {
                            palette[joints[v.bone_indices[i] as usize]]
                                .transform_point3(Vec3::from_array(v.position))
                                * (v.bone_weights[i] as f32 / total)
                        })
                        .sum::<Vec3>()
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
#[ignore = "requires retail GRFs configured in assets/convert.toml"]
fn retail_guild_flag_decodes_and_samples() {
    let bytes = retail_vfs()
        .read_asset("data/model/3dmob/guildflag90_1.gr2")
        .unwrap();
    let file = Gr2File::from_bytes(&bytes).expect("retail guild flag must decode");
    let model = &file.models[0];
    let skeleton = &file.skeletons[model.skeleton_index.unwrap()];
    println!(
        "models={} meshes={} joints={} vertices={} textures={} materials={} placement={:?}",
        file.models.len(),
        file.meshes.len(),
        skeleton.bones.len(),
        file.vertex_datas
            .iter()
            .map(|v| v.vertices.len())
            .sum::<usize>(),
        file.textures.len(),
        file.materials.len(),
        model.initial_placement
    );
    for texture in &file.textures {
        println!(
            "texture {} {}x{} encoding={} subformat={} alpha={}",
            texture.from_file_name,
            texture.width,
            texture.height,
            texture.encoding,
            texture.sub_format,
            texture.has_alpha()
        );
        assert_eq!(
            texture.to_rgba().unwrap().len(),
            (texture.width * texture.height * 4) as usize
        );
    }
    assert_eq!(
        file.textures
            .iter()
            .filter(|t| t.from_file_name.to_lowercase().contains("emblem"))
            .count(),
        1
    );
    let clip = &file.animations[0];
    println!(
        "idle={} duration={} step={} groups={:?}",
        clip.name, clip.duration, clip.time_step, clip.track_group_indices
    );
    for group in &file.track_groups {
        println!(
            "track group {} placement={:?}",
            group.name, group.initial_placement
        );
        for track in &group.transform_tracks {
            println!(
                "track {} curves={:?}",
                track.name,
                [&track.position, &track.orientation, &track.scale_shear].map(|c| (
                    c.degree,
                    c.knots.len(),
                    c.controls.len()
                ))
            );
        }
    }
    let first = posed_vertices(&file, &file, 0.0);
    let steps = (clip.duration * 120.0).ceil() as usize;
    let mut max_motion = 0.0_f32;
    for step in 0..=steps {
        let time = (step as f32 / 120.0).min(clip.duration);
        let vertices = posed_vertices(&file, &file, time);
        for (initial, current) in first.iter().zip(vertices) {
            max_motion = max_motion.max(initial.distance(current));
        }
    }
    println!("maximum vertex motion={max_motion}");
    assert!(max_motion > 1.0, "the original cloth must animate");
    let bounds = first.iter().fold(
        (glam::Vec3::splat(f32::MAX), glam::Vec3::splat(f32::MIN)),
        |(min, max), &p| (min.min(p), max.max(p)),
    );
    println!("source bounds={bounds:?}");
}

#[test]
#[ignore = "requires retail GRFs configured in assets/convert.toml"]
fn retail_gr2_corpus_decodes_and_samples() {
    use std::collections::BTreeMap;
    let vfs = retail_vfs();
    let files: BTreeMap<_, _> = source_paths(&vfs)
        .into_iter()
        .map(|path| {
            let file = Gr2File::from_bytes(&vfs.read_asset(&path).unwrap())
                .unwrap_or_else(|e| panic!("{path}: {e}"));
            (path, file)
        })
        .collect();
    assert!(!files.is_empty(), "no GR2 source files found");
    let mut used = std::collections::BTreeSet::new();
    for name in source_models(&vfs) {
        let path = format!("data/model/3dmob/{name}");
        let file = &files[&path];
        assert!(!file.meshes.is_empty(), "{name}: no geometry");
        for texture in &file.textures {
            assert_eq!(
                texture
                    .to_rgba()
                    .unwrap_or_else(|e| panic!("{name} texture {}: {e}", texture.from_file_name))
                    .len(),
                (texture.width * texture.height * 4) as usize
            );
        }
        let bone = name
            .strip_suffix(".gr2")
            .unwrap()
            .rsplit_once('_')
            .unwrap()
            .1;
        let mut clips = vec![(path.clone(), file)];
        for action in ["move", "attack", "damage", "dead"] {
            let clip_path = format!("data/model/3dmob_bone/{bone}_{action}.gr2");
            if let Some(clip) = files.get(&clip_path) {
                clips.push((clip_path, clip));
            }
        }
        for (clip_path, clip) in clips {
            used.insert(clip_path.clone());
            let duration = clip.animations[0].duration;
            let mut degrees = std::collections::BTreeSet::new();
            for group in &clip.track_groups {
                for track in &group.transform_tracks {
                    for curve in [&track.position, &track.orientation, &track.scale_shear] {
                        degrees.insert(curve.degree);
                    }
                }
            }
            for step in 0..=(duration * 120.0).ceil() as usize {
                let time = (step as f32 / 120.0).min(duration);
                let vertices = posed_vertices(file, clip, time);
                assert!(
                    vertices.iter().all(|v| v.is_finite()),
                    "{clip_path} nonfinite geometry"
                );
            }
            println!("{name}: {clip_path} duration={duration} degrees={degrees:?}");
        }
    }
    let unmatched: Vec<_> = files.keys().filter(|path| !used.contains(*path)).collect();
    println!("decoded {} GR2 files; unmatched={unmatched:?}", files.len());
    assert!(unmatched.is_empty(), "inspect unassociated GR2 files");
}
