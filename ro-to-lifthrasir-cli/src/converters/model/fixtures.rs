//! Synthetic prop fixtures shared by the model writer and validator tests: a
//! two-node RSM whose main node spans two textures and whose child references
//! a texture slot it does not have, plus an animated variant whose position
//! and rotation channels deliberately end on different frame numbers.

use crate::converters::map::fixtures::write_fixture_png;
use crate::converters::map::textures::TextureOut;
use crate::converters::model::mesh::{ModelBuild, build_model};
use crate::converters::model::writer::write_model_glb;
use ro_formats::{Face, Node, PosKeyframe, RotKeyframe, Rsm, ShadingType, TextureVertex};
use std::path::PathBuf;

pub const TEXTURES: [&str; 2] = ["bark.bmp", "leaf.bmp"];

pub fn textures() -> Vec<TextureOut> {
    TEXTURES
        .iter()
        .map(|name| TextureOut {
            source_name: (*name).to_string(),
            relative_path: format!("tex/{}.png", name.trim_end_matches(".bmp")),
        })
        .collect()
}

fn uv(u: f32, v: f32) -> TextureVertex {
    TextureVertex { color: None, u, v }
}

fn face(tex_id: u16) -> Face {
    Face {
        vertex_ids: [0, 1, 2],
        texture_vertex_ids: [0, 1, 2],
        tex_id,
        padding: 0,
        two_side: 0,
        smooth_group: 0,
    }
}

fn triangle_node(name: &str, texture_ids: Vec<i32>, faces: Vec<Face>) -> Node {
    Node {
        name: name.to_string(),
        parent_name: String::new(),
        texture_ids,
        mat3: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        offset: [0.0, 0.0, 0.0],
        pos: [0.0, 0.0, 0.0],
        rot_angle: 0.0,
        rot_axis: [0.0, 0.0, 0.0],
        scale: [1.0, 1.0, 1.0],
        vertices: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        texture_vertices: vec![uv(0.0, 0.0), uv(1.0, 0.0), uv(0.0, 1.0)],
        faces,
        pos_keyframes: Vec::new(),
        rot_keyframes: Vec::new(),
    }
}

/// Static two-node prop: `main` draws one triangle per exported texture,
/// `child` draws one whose texture slot resolves to `-1`.
pub fn textured_rsm() -> Rsm {
    let mut main = triangle_node("main", vec![0, 1], vec![face(0), face(1)]);
    main.pos = [1.0, 2.0, 3.0];
    main.rot_angle = std::f32::consts::FRAC_PI_2;
    main.rot_axis = [0.0, 1.0, 0.0];
    main.scale = [1.0, 2.0, 3.0];

    let mut child = triangle_node("child", Vec::new(), vec![face(0)]);
    child.parent_name = "main".to_string();
    child.pos = [4.0, 5.0, 6.0];

    Rsm {
        version: 1.4,
        anim_len: 0,
        shade_type: ShadingType::Smooth,
        alpha: 1.0,
        textures: TEXTURES.iter().map(|name| (*name).to_string()).collect(),
        main_node_name: "main".to_string(),
        nodes: vec![main, child],
        pos_keyframes: Vec::new(),
        volume_boxes: Vec::new(),
        bounding_box: None,
    }
}

/// The same prop animated over 4 seconds, with the position channel ending on
/// frame 8 and the rotation channel on frame 2 so the per-channel rescale is
/// observable.
pub fn animated_rsm() -> Rsm {
    let mut rsm = textured_rsm();
    rsm.anim_len = 4000;

    rsm.nodes[0].pos_keyframes = [0, 4, 8]
        .into_iter()
        .map(|frame| PosKeyframe {
            frame,
            px: frame as f32,
            py: 0.0,
            pz: 0.0,
        })
        .collect();
    rsm.nodes[1].rot_keyframes = vec![
        RotKeyframe {
            frame: 0,
            q: [0.0, 0.0, 0.0, 1.0],
        },
        RotKeyframe {
            frame: 2,
            q: [0.0, 1.0, 0.0, 0.0],
        },
    ];

    rsm
}

/// A tempdir with the fixture PNGs already exported beside where the glb goes,
/// so a written glb reimports with its images resolved.
pub fn fixture_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    for texture in textures() {
        write_fixture_png(dir.path(), &texture.relative_path);
    }
    dir
}

pub struct ModelFixture {
    pub _dir: tempfile::TempDir,
    pub path: PathBuf,
    pub rsm: Rsm,
    pub build: ModelBuild,
}

impl ModelFixture {
    pub const RSM_HASH: &'static str = "cafebabe";
}

/// Writes `tree01.glb` from [`textured_rsm`].
pub fn write_fixture() -> ModelFixture {
    let dir = fixture_dir();
    let path = dir.path().join("tree01.glb");
    let rsm = textured_rsm();
    let build = build_model(&rsm).expect("model must build");

    write_model_glb(&path, &rsm, ModelFixture::RSM_HASH, &build, &textures()).expect("write glb");

    ModelFixture {
        _dir: dir,
        path,
        rsm,
        build,
    }
}
