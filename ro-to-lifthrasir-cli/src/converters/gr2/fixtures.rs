//! Synthetic two-joint model, independent of the retail archives.

use ro_formats::gr2::{Gr2File, model::*};

pub(super) fn model() -> Gr2File {
    let mut root = Gr2Transform::IDENTITY;
    root.position = [2.0, 0.0, 0.0];
    let mut child = Gr2Transform::IDENTITY;
    child.position = [0.0, 3.0, 0.0];
    Gr2File {
        textures: vec![Gr2Texture {
            from_file_name: "synthetic.png".into(),
            width: 1,
            height: 1,
            encoding: TEXTURE_ENCODING_RAW,
            sub_format: 0,
            bytes_per_pixel: 4,
            component_bits: [8; 4],
            pixels: vec![255; 4],
        }],
        materials: vec![Gr2Material {
            name: "surface".into(),
            texture_index: Some(0),
        }],
        skeletons: vec![Gr2Skeleton {
            name: "skeleton".into(),
            bones: vec![
                Gr2Bone {
                    name: "root".into(),
                    parent_index: -1,
                    transform: root,
                    inverse_world: glam::Mat4::from_translation(glam::Vec3::new(-2.0, 0.0, 0.0))
                        .to_cols_array(),
                },
                Gr2Bone {
                    name: "child".into(),
                    parent_index: 0,
                    transform: child,
                    inverse_world: glam::Mat4::from_translation(glam::Vec3::new(-2.0, -3.0, 0.0))
                        .to_cols_array(),
                },
            ],
        }],
        vertex_datas: vec![Gr2VertexData {
            has_bone_weights: true,
            vertices: [[2.0, 3.0, 1.0], [3.0, 3.0, 1.0], [2.0, 4.0, 1.0]]
                .map(|position| Gr2Vertex {
                    position,
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 0.0],
                    bone_weights: [255, 0, 0, 0],
                    bone_indices: [0; 4],
                })
                .to_vec(),
        }],
        tri_topologies: vec![Gr2TriTopology {
            groups: vec![Gr2TriGroup {
                material_index: 0,
                tri_first: 0,
                tri_count: 1,
            }],
            indices: vec![0, 1, 2],
        }],
        meshes: vec![Gr2Mesh {
            name: "triangle".into(),
            vertex_data_index: Some(0),
            topology_index: Some(0),
            material_indices: vec![0],
            bone_bindings: vec!["child".into(), "root".into()],
        }],
        models: vec![Gr2Model {
            name: "synthetic".into(),
            skeleton_index: Some(0),
            initial_placement: Gr2Transform::IDENTITY,
            mesh_indices: vec![0],
        }],
        track_groups: vec![Gr2TrackGroup {
            name: "skeleton".into(),
            initial_placement: Gr2Transform::IDENTITY,
            transform_tracks: vec![Gr2TransformTrack {
                name: "child".into(),
                position: Gr2Curve {
                    degree: 0,
                    knots: vec![0.0],
                    controls: vec![0.0, 5.0, 0.0],
                },
                orientation: Gr2Curve::default(),
                scale_shear: Gr2Curve::default(),
            }],
        }],
        animations: vec![Gr2Animation {
            name: "test".into(),
            duration: 1.0,
            time_step: 1.0 / 30.0,
            track_group_indices: vec![0],
        }],
    }
}
