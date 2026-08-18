//! Synthetic map fixtures shared by the writer and validator tests: a 2x2
//! ground with one texture, an RSW carrying one of every object kind, and a
//! raw GAT buffer in the real on-disk layout.

use crate::converters::ktx2_out::encode_ktx2;
use crate::converters::map::terrain::{TerrainPrimitive, build_terrain};
use crate::converters::map::textures::TextureOut;
use crate::converters::map::writer::{MapGlbInputs, write_glb};
use image::{Rgba, RgbaImage};
use lifthrasir_data::lif;
use ro_formats::{
    GndSurface, GndTile, RoGround, RoWorld, RswEffect, RswGround, RswLight, RswLightObj, RswModel,
    RswObject, RswSound, RswWater,
};
use std::collections::HashSet;
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
        raw_version: 0x0107,
        width: 2,
        height: 2,
        textures: vec![TEXTURE.into()],
        texture_indexes: vec![0],
        tiles: vec![tile],
        surfaces: vec![surface.clone(), surface.clone(), surface.clone(), surface],
        water: None,
    }
}

pub fn mini_world() -> RoWorld {
    RoWorld {
        version: "2.2".into(),
        raw_version: 0x0202,
        build_number: 0,
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
                anim_type: 1,
                anim_speed: 2.0,
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
        relative_path: "tex/grass01.ktx2".to_string(),
    }]
}

pub fn write_fixture_ktx2(dir: &Path, relative: &str) {
    let path = dir.join(relative);
    std::fs::create_dir_all(path.parent().expect("tex dir")).expect("create tex dir");
    let bytes = encode_ktx2(&RgbaImage::new(1, 1), true).expect("encode KTX2");
    std::fs::write(path, bytes).expect("write KTX2");
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
    pub converted_models: HashSet<String>,
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
            converted_models: &self.converted_models,
            indoor: false,
        }
    }
}

/// Writes `mini.glb` (plus its texture KTX2) into a temp dir and hands back
/// everything it was built from.
pub fn write_fixture() -> Fixture {
    let dir = tempfile::tempdir().expect("tempdir");
    write_fixture_ktx2(dir.path(), "tex/grass01.ktx2");

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
        converted_models: HashSet::from(["prontera\\tree01.rsm".to_string()]),
    };
    write_glb(&fixture.path, &fixture.inputs()).expect("write glb");
    fixture
}

/// Regenerates the binary fixtures that `game-engine`'s `LIF_*` extension
/// handler is tested against. They are committed because the runtime crate has
/// no way to build a glb (this converter is a binary, not a library), and this
/// helper is what documents where they came from.
///
/// ```text
/// cargo test -p ro-to-lifthrasir-cli -- --ignored regenerate_game_engine_fixtures
/// ```
#[test]
#[ignore = "rewrites committed fixtures under game-engine/tests/fixtures"]
fn regenerate_game_engine_fixtures() {
    let out = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join("game-engine/tests/fixtures");
    std::fs::create_dir_all(&out).expect("create fixtures dir");
    write_fixture_ktx2(&out, "tex/grass01.ktx2");

    let mut ground = mini_ground();
    ground.surfaces[1].height = [12.0; 4];
    ground.surfaces[2].height = [12.0; 4];
    let world = mini_world();
    let gat = raw_gat(ground.width * 2, ground.height * 2);
    let primitives = build_terrain(&ground).expect("terrain");
    let good = out.join("mini_map.glb");

    write_glb(
        &good,
        &MapGlbInputs {
            map_name: "mini_map",
            ground: &ground,
            world: &world,
            primitives: &primitives,
            textures: &textures(),
            gat_bytes: &gat,
            gnd_bytes: b"gnd-bytes",
            rsw_bytes: b"rsw-bytes",
            converted_models: &HashSet::from(["prontera\\tree01.rsm".to_string()]),
            indoor: false,
        },
    )
    .expect("write mini_map.glb");

    // The same document declaring a format the runtime does not support.
    let version = patched_glb(
        &good,
        &format!("\"format_version\":{}", lif::FORMAT_VERSION),
        &format!("\"format_version\":{}", lif::FORMAT_VERSION + 1),
    );
    std::fs::write(out.join("bad_version.glb"), version).expect("write bad_version.glb");

    // The same document with a `lif_audio` field renamed out from under the
    // runtime's schema.
    let extras = patched_glb(&good, "\"volume\"", "\"volumr\"");
    std::fs::write(out.join("bad_extras.glb"), extras).expect("write bad_extras.glb");

    // The same document with only its water extension renamed away: a map that
    // simply has no water.
    let dry = patched_glb(&good, lif::EXTENSION_WATER, "XIF_water");
    std::fs::write(out.join("no_water.glb"), dry).expect("write no_water.glb");

    // Keep the one-byte mask but declare water dimensions that require three.
    let bad_water_mask = patched_glb(
        &good,
        "\"split_width\":1,\"width\":2",
        "\"split_width\":1,\"width\":9",
    );
    std::fs::write(out.join("bad_water_mask.glb"), bad_water_mask)
        .expect("write bad_water_mask.glb");

    // The same document with every `LIF_` prefix renamed away: a perfectly
    // ordinary glTF that the runtime's handler must not touch at all.
    let plain = patched_glb(&good, "LIF_", "XIF_");
    std::fs::write(out.join("plain.glb"), plain).expect("write plain.glb");
}

/// Regenerates the multi-level KTX2 image used by the runtime loader test.
///
/// ```text
/// cargo test -p ro-to-lifthrasir-cli -- --ignored generate_mip_probe_fixture
/// ```
#[test]
#[ignore = "rewrites committed fixture under game-engine/tests/fixtures"]
fn generate_mip_probe_fixture() {
    let out = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join("game-engine/tests/fixtures/tex/mip_probe.ktx2");
    let image = RgbaImage::from_fn(4, 4, |x, y| match (x < 2, y < 2) {
        (true, true) => Rgba([255, 0, 0, 255]),
        (false, true) => Rgba([0, 255, 0, 255]),
        (true, false) => Rgba([0, 0, 255, 255]),
        (false, false) => Rgba([255, 255, 255, 255]),
    });
    let bytes = encode_ktx2(&image, true).expect("encode mip probe KTX2");

    std::fs::create_dir_all(out.parent().expect("tex dir")).expect("create tex dir");
    std::fs::write(out, bytes).expect("write mip probe KTX2");
}

/// Rewrites every occurrence of `needle` in a glb's JSON chunk. Only
/// equal-length replacements are allowed, so the chunk lengths stay valid
/// without re-serializing the document; the binary chunk is left alone so a
/// short needle cannot corrupt embedded data.
fn patched_glb(source: &Path, needle: &str, replacement: &str) -> Vec<u8> {
    assert_eq!(
        needle.len(),
        replacement.len(),
        "glb patches must not change length: '{needle}' -> '{replacement}'"
    );

    let mut bytes = std::fs::read(source).expect("read glb");
    // GLB layout: a 12-byte header, then an 8-byte chunk header before the JSON.
    let json_length =
        u32::from_le_bytes(bytes[12..16].try_into().expect("json chunk length")) as usize;
    let json_end = 20 + json_length;

    let mut hits = 0;
    let mut at = 20;
    while at + needle.len() <= json_end {
        if &bytes[at..at + needle.len()] == needle.as_bytes() {
            bytes[at..at + needle.len()].copy_from_slice(replacement.as_bytes());
            hits += 1;
            at += needle.len();
            continue;
        }
        at += 1;
    }
    assert!(
        hits > 0,
        "'{needle}' not found in the JSON chunk of {}",
        source.display()
    );

    bytes
}
