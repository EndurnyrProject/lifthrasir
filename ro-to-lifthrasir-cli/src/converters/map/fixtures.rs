//! Synthetic map fixtures shared by the writer and validator tests: a 2x2
//! ground with one texture, an RSW carrying one of every object kind, and a
//! raw GAT buffer in the real on-disk layout.

use crate::converters::map::terrain::{TerrainPrimitive, build_terrain};
use crate::converters::map::textures::TextureOut;
use crate::converters::map::writer::{MapGlbInputs, write_glb};
use image::{ImageFormat, RgbaImage};
use ro_formats::{
    GndSurface, GndTile, RoGround, RoWorld, RswEffect, RswGround, RswLight, RswLightObj, RswModel,
    RswObject, RswSound, RswWater,
};
use std::io::Cursor;
use std::path::{Path, PathBuf};

pub const TEXTURE: &str = "grass01.bmp";

pub fn mini_ground() -> RoGround {
    let tile = GndTile {
        u1: 0.0,
        u2: 1.0,
        u3: 0.0,
        u4: 1.0,
        v1: 0.0,
        v2: 0.0,
        v3: 1.0,
        v4: 1.0,
        texture: 0,
        color: [255, 255, 255, 255],
    };
    let surface = GndSurface {
        height: [-10.0, -10.0, -10.0, -10.0],
        tile_up: 0,
        tile_front: -1,
        tile_right: -1,
    };

    RoGround {
        version: "1.7".into(),
        width: 2,
        height: 2,
        textures: vec![TEXTURE.into()],
        texture_indexes: vec![0],
        tiles: vec![tile],
        surfaces: vec![surface.clone(), surface.clone(), surface.clone(), surface],
    }
}

pub fn mini_world() -> RoWorld {
    RoWorld {
        version: "2.2".into(),
        ini_file: String::new(),
        gnd_file: "mini.gnd".into(),
        gat_file: "mini.gat".into(),
        src_file: None,
        water: RswWater {
            level: 12.5,
            water_type: 3,
            wave_height: 0.6,
            wave_speed: 1.25,
            wave_pitch: 45.0,
            anim_speed: 4,
        },
        light: RswLight {
            longitude: 45,
            latitude: 30,
            diffuse: [0.9, 0.8, 0.7],
            ambient: [0.25, 0.3, 0.35],
            opacity: 0.5,
        },
        ground: RswGround::default(),
        objects: vec![
            RswObject::Light(RswLightObj {
                name: "torch".into(),
                position: [10.0, -5.0, 20.0],
                color: [1.0, 0.5, 0.25],
                range: 40.0,
            }),
            RswObject::Sound(RswSound {
                name: "water".into(),
                wav_file: "effect\\water.wav".into(),
                position: [1.0, -2.0, 3.0],
                volume: 0.75,
                width: 1,
                height: 1,
                range: 30.0,
                cycle: 4.5,
            }),
            RswObject::Effect(RswEffect {
                name: "steam".into(),
                position: [4.0, -6.0, 8.0],
                effect_type: 12,
                emit_speed: 0.25,
                params: [1.0, 2.0, 3.0, 4.0],
            }),
            RswObject::Model(RswModel {
                name: "tree".into(),
                anim_type: 0,
                anim_speed: 1.0,
                block_type: 0,
                filename: "prontera\\tree01.rsm".into(),
                node_name: String::new(),
                position: [7.0, -8.0, 9.0],
                rotation: [10.0, 20.0, 30.0],
                scale: [1.0, 2.0, 3.0],
            }),
        ],
    }
}

/// A raw `.gat` buffer in the on-disk layout (`GRAT` + version + dims +
/// per-cell `[f32; 4]` heights and a `u32` cell type).
pub fn raw_gat(width: u32, height: u32) -> Vec<u8> {
    let mut buffer = Vec::new();
    buffer.extend_from_slice(b"GRAT");
    buffer.push(1);
    buffer.push(2);
    buffer.extend_from_slice(&width.to_le_bytes());
    buffer.extend_from_slice(&height.to_le_bytes());
    for i in 0..(width * height) {
        for offset in 0..4 {
            buffer.extend_from_slice(&((i + offset) as f32).to_le_bytes());
        }
        buffer.extend_from_slice(&1u32.to_le_bytes());
    }
    buffer
}

pub fn textures() -> Vec<TextureOut> {
    vec![TextureOut {
        source_name: TEXTURE.to_string(),
        relative_path: "tex/grass01.png".to_string(),
    }]
}

pub fn write_fixture_png(dir: &Path, relative: &str) {
    let path = dir.join(relative);
    std::fs::create_dir_all(path.parent().expect("tex dir")).expect("create tex dir");
    let mut bytes = Vec::new();
    RgbaImage::new(1, 1)
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .expect("encode png");
    std::fs::write(path, bytes).expect("write png");
}

pub struct Fixture {
    pub _dir: tempfile::TempDir,
    pub path: PathBuf,
    pub ground: RoGround,
    pub world: RoWorld,
    pub gat: Vec<u8>,
    pub gnd_bytes: Vec<u8>,
    pub rsw_bytes: Vec<u8>,
    pub primitives: Vec<TerrainPrimitive>,
    pub textures: Vec<TextureOut>,
}

impl Fixture {
    pub fn inputs(&self) -> MapGlbInputs<'_> {
        MapGlbInputs {
            map_name: "mini",
            ground: &self.ground,
            world: &self.world,
            primitives: &self.primitives,
            textures: &self.textures,
            gat_bytes: &self.gat,
            gnd_bytes: &self.gnd_bytes,
            rsw_bytes: &self.rsw_bytes,
        }
    }
}

/// Writes `mini.glb` (plus its texture PNG) into a temp dir and hands back
/// everything it was built from.
pub fn write_fixture() -> Fixture {
    let dir = tempfile::tempdir().expect("tempdir");
    write_fixture_png(dir.path(), "tex/grass01.png");

    let ground = mini_ground();
    let world = mini_world();
    let gat = raw_gat(ground.width * 2, ground.height * 2);
    let primitives = build_terrain(&ground).expect("terrain");
    let path = dir.path().join("mini.glb");

    let fixture = Fixture {
        _dir: dir,
        path,
        ground,
        world,
        gat,
        gnd_bytes: b"gnd-bytes".to_vec(),
        rsw_bytes: b"rsw-bytes".to_vec(),
        primitives,
        textures: textures(),
    };
    write_glb(&fixture.path, &fixture.inputs()).expect("write glb");
    fixture
}
