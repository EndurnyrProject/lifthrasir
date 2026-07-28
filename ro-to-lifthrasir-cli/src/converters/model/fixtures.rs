//! Synthetic prop fixtures shared by the model writer and validator tests: a
//! two-node RSM whose main node spans two textures and whose child references
//! a texture slot it does not have, plus an animated variant whose position
//! and rotation channels deliberately end on different frame numbers.

use crate::converters::map::fixtures::write_fixture_png;
use crate::converters::map::textures::TextureOut;
use crate::converters::model::mesh::build_model;
use crate::converters::model::normalized::{NormalizedModel, ShadingPolicy};
use crate::converters::model::writer::write_model_glb;
use crate::grf_vfs::AssetRead;
use image::{ImageFormat, RgbaImage};
use lifthrasir_data::lif::{LifScalarKey, LifUvAnimation, LifUvChannel, LifUvProperty};
use ro_formats::{Face, Node, PosKeyframe, RotKeyframe, Rsm, ShadingType, TextureVertex};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Cursor;
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
pub fn uv_only_model(scale: [f32; 2], rotation: f32) -> NormalizedModel {
    let mut model = build_model(&textured_rsm(), ModelFixture::RSM_HASH).expect("model must build");
    model.duration_ms = 1000.0;
    let animation = LifUvAnimation {
        duration_ms: 1000,
        channels: vec![
            LifUvChannel {
                property: LifUvProperty::ScaleU,
                keys: vec![LifScalarKey {
                    time_ms: 0,
                    value: scale[0],
                }],
            },
            LifUvChannel {
                property: LifUvProperty::ScaleV,
                keys: vec![LifScalarKey {
                    time_ms: 0,
                    value: scale[1],
                }],
            },
            LifUvChannel {
                property: LifUvProperty::Rotate,
                keys: vec![LifScalarKey {
                    time_ms: 0,
                    value: rotation,
                }],
            },
        ],
    };
    let primitive = &mut model.nodes[0].primitives[0];
    let source = primitive.uv0.clone();
    let matrix = animation.sample(0, false).unwrap().matrix3();
    primitive.uv0 = source
        .iter()
        .map(|uv| {
            [
                matrix[0] * uv[0] + matrix[1] * uv[1] + matrix[2],
                matrix[3] * uv[0] + matrix[4] * uv[1] + matrix[5],
            ]
        })
        .collect();
    primitive.uv1 = Some(source);
    primitive.uv_animation = Some(animation);
    model.materials[primitive.material].shading = ShadingPolicy::None;
    model
}

pub fn fixture_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    for texture in textures() {
        write_fixture_png(dir.path(), &texture.relative_path);
    }
    dir
}

/// In-memory stand-in for `GrfVfs`, counting reads so the texture pool's
/// write-once behaviour is observable.
#[derive(Default)]
pub struct FakeVfs {
    files: HashMap<String, Vec<u8>>,
    reads: RefCell<HashMap<String, usize>>,
}

impl FakeVfs {
    pub fn with(files: &[(&str, Vec<u8>)]) -> Self {
        Self {
            files: files
                .iter()
                .map(|(path, bytes)| ((*path).to_string(), bytes.clone()))
                .collect(),
            reads: RefCell::new(HashMap::new()),
        }
    }

    pub fn reads(&self, logical_path: &str) -> usize {
        self.reads.borrow().get(logical_path).copied().unwrap_or(0)
    }
}

impl AssetRead for FakeVfs {
    fn read_asset(&self, logical_path: &str) -> Option<Vec<u8>> {
        *self
            .reads
            .borrow_mut()
            .entry(logical_path.to_string())
            .or_default() += 1;
        self.files.get(logical_path).cloned()
    }
}

/// A 1x1 opaque BMP of `color`, the smallest thing the texture pool can
/// normalize. Distinct colors give distinct pooled PNGs, which is what the
/// pool's cross-run collision check compares.
pub fn bmp_bytes(color: [u8; 4]) -> Vec<u8> {
    let mut image = RgbaImage::new(1, 1);
    image.put_pixel(0, 0, image::Rgba(color));

    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Bmp)
        .expect("encode bmp");
    bytes
}

/// Encodes an RSM the `ro_formats` parser reads back: one node drawing a
/// triangle per declared texture. `version` is written verbatim so unsupported
/// revisions can be exercised without a real RSM2 body.
pub fn encode_rsm(version: (u8, u8), textures: &[&str]) -> Vec<u8> {
    let (major, minor) = version;
    let mut bytes = b"GRSM".to_vec();
    bytes.extend([major, minor]);
    bytes.extend(0i32.to_le_bytes());
    bytes.extend(2i32.to_le_bytes());
    bytes.push(255);
    bytes.extend([0u8; 16]);

    bytes.extend((textures.len() as i32).to_le_bytes());
    for texture in textures {
        bytes.extend(fixed_string(texture));
    }
    bytes.extend(fixed_string("main"));
    bytes.extend(1i32.to_le_bytes());

    bytes.extend(fixed_string("main"));
    bytes.extend(fixed_string(""));
    bytes.extend((textures.len() as i32).to_le_bytes());
    for index in 0..textures.len() {
        bytes.extend((index as i32).to_le_bytes());
    }
    extend_f32(&mut bytes, &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
    extend_f32(&mut bytes, &[0.0, 0.0, 0.0]);
    extend_f32(&mut bytes, &[0.0, 0.0, 0.0]);
    extend_f32(&mut bytes, &[0.0]);
    extend_f32(&mut bytes, &[0.0, 1.0, 0.0]);
    extend_f32(&mut bytes, &[1.0, 1.0, 1.0]);

    bytes.extend(3i32.to_le_bytes());
    extend_f32(&mut bytes, &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);

    bytes.extend(3i32.to_le_bytes());
    for (u, v) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)] {
        bytes.extend([255, 255, 255, 255]);
        extend_f32(&mut bytes, &[u, v]);
    }

    bytes.extend((textures.len() as i32).to_le_bytes());
    for tex_id in 0..textures.len() as u16 {
        for value in [0u16, 1, 2, 0, 1, 2, tex_id, 0] {
            bytes.extend(value.to_le_bytes());
        }
        bytes.extend(0i32.to_le_bytes());
        bytes.extend(0i32.to_le_bytes());
    }

    bytes.extend(0i32.to_le_bytes());
    bytes.extend(0i32.to_le_bytes());
    bytes.extend(0i32.to_le_bytes());
    bytes
}

pub fn encode_rsm2(minor: u8, texture: &str) -> Vec<u8> {
    assert!(matches!(minor, 2 | 3));
    let mut bytes = b"GRSM".to_vec();
    bytes.extend([2, minor]);
    bytes.extend(0i32.to_le_bytes());
    bytes.extend(2i32.to_le_bytes());
    bytes.push(255);
    bytes.extend(30.0f32.to_le_bytes());
    if minor == 2 {
        bytes.extend(1i32.to_le_bytes());
        dynamic_string(&mut bytes, texture);
    }
    bytes.extend(1i32.to_le_bytes());
    dynamic_string(&mut bytes, "main");
    bytes.extend(1i32.to_le_bytes());
    dynamic_string(&mut bytes, "main");
    dynamic_string(&mut bytes, "");
    bytes.extend(1i32.to_le_bytes());
    if minor == 2 {
        bytes.extend(0i32.to_le_bytes());
    } else {
        dynamic_string(&mut bytes, texture);
    }
    extend_f32(&mut bytes, &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
    extend_f32(&mut bytes, &[0.0, 0.0, 0.0]);
    bytes.extend(3i32.to_le_bytes());
    extend_f32(&mut bytes, &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    bytes.extend(3i32.to_le_bytes());
    for (u, v) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)] {
        extend_f32(&mut bytes, &[0.0, u, v]);
    }
    bytes.extend(1i32.to_le_bytes());
    bytes.extend(32i32.to_le_bytes());
    for value in [0u16, 1, 2, 0, 1, 2, 0, 0] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend(0i32.to_le_bytes());
    for value in [1i32, 1, 1] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend(0i32.to_le_bytes());
    bytes.extend(0i32.to_le_bytes());
    bytes.extend(0i32.to_le_bytes());
    if minor == 3 {
        bytes.extend(0i32.to_le_bytes());
    }
    bytes.extend(0i32.to_le_bytes());
    bytes
}

fn dynamic_string(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend((value.len() as i32).to_le_bytes());
    bytes.extend(value.as_bytes());
}

fn fixed_string(value: &str) -> [u8; 40] {
    let mut field = [0u8; 40];
    field[..value.len()].copy_from_slice(value.as_bytes());
    field
}

fn extend_f32(bytes: &mut Vec<u8>, values: &[f32]) {
    for value in values {
        bytes.extend(value.to_le_bytes());
    }
}

pub struct ModelFixture {
    pub _dir: tempfile::TempDir,
    pub path: PathBuf,
    pub model: NormalizedModel,
}

impl ModelFixture {
    pub const RSM_HASH: &'static str = "cafebabe";
}

/// Writes `tree01.glb` from [`textured_rsm`].
pub fn write_fixture() -> ModelFixture {
    let dir = fixture_dir();
    let path = dir.path().join("tree01.glb");
    let rsm = textured_rsm();
    let model = build_model(&rsm, ModelFixture::RSM_HASH).expect("model must build");

    write_model_glb(&path, &model, &textures()).expect("write glb");

    ModelFixture {
        _dir: dir,
        path,
        model,
    }
}
