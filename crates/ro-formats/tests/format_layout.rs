//! Synthetic layout fixtures for the RSM1, RSW and GND readers.
//!
//! These build byte streams by hand so each version gate can be exercised in CI
//! without shipping Gravity's retail data, which is deliberately kept out of the
//! repository. They pin the layout the parsers agree to read.
//!
//! They cannot, on their own, prove the layout matches reality - a fixture
//! written from a wrong belief would agree with a parser holding that same wrong
//! belief. That is what `retail_corpus.rs` is for: it sweeps the real archives
//! and is the check that catches a mistaken assumption. Keep both.

use ro_formats::{RoGround, RoWorld, Rsm};

// ---------------------------------------------------------------- byte writer

#[derive(Default)]
struct Buf(Vec<u8>);

impl Buf {
    fn u8(&mut self, v: u8) -> &mut Self {
        self.0.push(v);
        self
    }
    fn i32(&mut self, v: i32) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn u32(&mut self, v: u32) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn u16(&mut self, v: u16) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn f32(&mut self, v: f32) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn f32s(&mut self, vs: &[f32]) -> &mut Self {
        vs.iter().for_each(|v| {
            self.f32(*v);
        });
        self
    }
    fn raw(&mut self, bytes: &[u8]) -> &mut Self {
        self.0.extend_from_slice(bytes);
        self
    }
    fn zeros(&mut self, n: usize) -> &mut Self {
        self.0.resize(self.0.len() + n, 0);
        self
    }
    /// Fixed-width, NUL-padded name field.
    fn name(&mut self, s: &str, width: usize) -> &mut Self {
        let bytes = s.as_bytes();
        assert!(bytes.len() < width);
        self.raw(bytes).zeros(width - bytes.len())
    }
}

// ------------------------------------------------------------------ RSM1

/// A one-node RSM1 with `rot_keys` rotation keyframes on that node and
/// `scale_keys` model-wide trailing scale keyframes.
fn rsm(major: u8, minor: u8, rot_keys: &[i32], scale_keys: &[i32]) -> Vec<u8> {
    let version = ((major as u16) << 8) | minor as u16;
    let mut b = Buf::default();
    b.raw(b"GRSM").u8(major).u8(minor);
    b.i32(1000).i32(2); // anim_len, shade_type = Smooth
    if version >= 0x0104 {
        b.u8(255); // alpha
    }
    b.zeros(16); // reserved

    b.i32(1).name("tex.bmp", 40); // one texture
    b.name("root", 40); // main node name
    b.i32(1); // one node

    b.name("root", 40).name("", 40);
    b.i32(1).i32(0); // one texture id
    b.f32s(&[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]); // mat3
    b.f32s(&[0.0, 0.0, 0.0]); // offset
    b.f32s(&[0.0, 0.0, 0.0]); // pos
    b.f32(0.0).f32s(&[0.0, 1.0, 0.0]); // rot angle + axis
    b.f32s(&[1.0, 1.0, 1.0]); // scale

    b.i32(3); // vertices
    b.f32s(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]);

    b.i32(3); // texture vertices
    for _ in 0..3 {
        if version >= 0x0102 {
            b.raw(&[255, 255, 255, 255]);
        }
        b.f32(0.0).f32(0.0);
    }

    b.i32(1); // one face
    b.u16(0).u16(1).u16(2).u16(0).u16(1).u16(2);
    b.u16(0).u16(0).i32(0);
    if version >= 0x0102 {
        b.i32(0); // smooth group
    }

    // Rotation keyframes: always present, 20 bytes each. There is no per-node
    // position keyframe block in RSM1 at any version.
    b.i32(rot_keys.len() as i32);
    for &frame in rot_keys {
        b.i32(frame).f32s(&[0.0, 0.0, 0.0, 1.0]);
    }

    // Model-wide scale keyframes, below 1.6, 20 bytes each.
    if version < 0x0106 {
        b.i32(scale_keys.len() as i32);
        for &frame in scale_keys {
            b.i32(frame).f32s(&[1.0, 1.0, 1.0]).f32(0.0);
        }
    }

    b.i32(0); // no volume boxes

    if version >= 0x0105 {
        b.zeros(4); // four trailing bytes no implementation reads
    }
    b.0
}

#[test]
fn rsm_1_4_reads_node_rotation_keys_and_trailing_scale_keys() {
    let model = Rsm::from_bytes(&rsm(1, 4, &[0, 10, 20], &[0, 5])).expect("1.4 must parse");

    assert_eq!(model.raw_version, 0x0104);
    assert_eq!(model.nodes.len(), 1);
    assert_eq!(
        model.nodes[0]
            .rot_keyframes
            .iter()
            .map(|k| k.frame)
            .collect::<Vec<_>>(),
        vec![0, 10, 20]
    );
    assert_eq!(model.scale_keyframes.len(), 2);
}

#[test]
fn rsm_1_5_reads_the_same_node_layout_as_1_4() {
    // The old reader took a position-keyframe block here at >= 1.5, which shifted
    // every following field and ran off the end of the file.
    let model = Rsm::from_bytes(&rsm(1, 5, &[0, 4, 8], &[])).expect("1.5 must parse");

    assert_eq!(model.raw_version, 0x0105);
    assert_eq!(
        model.nodes[0]
            .rot_keyframes
            .iter()
            .map(|k| k.frame)
            .collect::<Vec<_>>(),
        vec![0, 4, 8]
    );
}

#[test]
fn rsm_rejects_a_negative_count() {
    // A negative count is what a desynchronised read lands on. `0..count` is an
    // empty range for a negative count, so this used to parse as "no textures".
    let mut bytes = rsm(1, 4, &[], &[]);
    let texture_count = 4 + 2 + 4 + 4 + 1 + 16;
    bytes[texture_count..texture_count + 4].copy_from_slice(&(-3i32).to_le_bytes());

    assert!(Rsm::from_bytes(&bytes).is_err(), "negative count must fail");
}

#[test]
fn rsm_rejects_unconsumed_trailing_bytes() {
    let mut bytes = rsm(1, 4, &[], &[]);
    bytes.extend_from_slice(&[0; 8]);

    let err = Rsm::from_bytes(&bytes).expect_err("trailing bytes must fail");
    assert!(
        err.to_string().contains("unconsumed"),
        "unexpected error: {err}"
    );
}

// ------------------------------------------------------------------- RSW

/// A minimal RSW carrying one model object.
fn rsw(major: u8, minor: u8, build_number: i32) -> Vec<u8> {
    let version = ((major as u16) << 8) | minor as u16;
    let mut b = Buf::default();
    b.raw(b"GRSW").u8(major).u8(minor);

    if version >= 0x0205 {
        b.i32(build_number);
    }
    if version >= 0x0202 {
        b.u8(0);
    }

    b.name("map.ini", 40).name("map.gnd", 40);
    if version > 0x0104 {
        b.name("map.gat", 40);
    }
    b.name("map.ini", 40); // the client writes the ini name a second time

    // Absent from 2.6 onward: the water block moved into the GND.
    if version < 0x0206 {
        if version >= 0x0103 {
            b.f32(-5.0);
        }
        if version >= 0x0108 {
            b.u32(1).f32(0.2).f32(2.0).f32(50.0);
        }
        if version >= 0x0109 {
            b.u32(3);
        }
    }

    if version >= 0x0105 {
        b.u32(45).u32(45).f32s(&[1.0, 1.0, 1.0]).f32s(&[0.3; 3]);
    }
    if version >= 0x0107 {
        b.f32(1.0);
    }
    if version >= 0x0106 {
        b.i32(-500).i32(500).i32(-500).i32(500);
    }
    if version >= 0x0207 {
        b.i32(0); // unknown int array, empty
    }

    b.i32(1); // one object
    b.i32(1); // type 1 = model
    if version >= 0x0103 {
        b.name("prop", 40).u32(0).f32(1.0).u32(0);
    }
    if version >= 0x0206 && build_number >= 186 {
        b.u8(0);
    }
    if version >= 0x0207 {
        b.i32(-1);
    }
    b.name("prop.rsm", 80).name("", 80);
    b.f32s(&[1.0, 2.0, 3.0]) // position
        .f32s(&[0.0, 0.0, 0.0]) // rotation
        .f32s(&[1.0, 1.0, 1.0]); // scale

    if version >= 0x0201 {
        b.zeros(1365 * 4 * 3 * 4); // quadtree
    }
    b.0
}

fn model_position(world: &RoWorld) -> [f32; 3] {
    match &world.objects[0] {
        ro_formats::RswObject::Model(model) => model.position,
        other => panic!("expected a model object, got {other:?}"),
    }
}

#[test]
fn rsw_reads_every_version_gate() {
    // 2.2 added a pad byte, 2.5 a build number, 2.6 dropped the water block and
    // 2.7 added an int array. Each was previously missing, so every field after
    // byte 6 was misaligned and positions decoded as garbage - occasionally NaN.
    for (major, minor, build) in [
        (1, 9, 0),
        (2, 1, 0),
        (2, 2, 0),
        (2, 5, 100),
        (2, 6, 0),   // build < 186: no extra model byte
        (2, 6, 186), // build >= 186: extra model byte
        (2, 7, 200),
    ] {
        let world = RoWorld::from_bytes(&rsw(major, minor, build))
            .unwrap_or_else(|e| panic!("{major}.{minor} (build {build}) must parse: {e}"));

        assert_eq!(world.gnd_file, "map.gnd", "{major}.{minor}");
        assert_eq!(world.gat_file, "map.gat", "{major}.{minor}");
        assert_eq!(world.objects.len(), 1, "{major}.{minor}");
        assert_eq!(
            model_position(&world),
            [1.0, 2.0, 3.0],
            "{major}.{minor} decoded a misaligned position"
        );
    }
}

#[test]
fn rsw_2_6_model_byte_is_gated_on_the_build_number_not_the_version() {
    // Same version, different build: the model record differs by one byte.
    let low = RoWorld::from_bytes(&rsw(2, 6, 185)).expect("build 185");
    let high = RoWorld::from_bytes(&rsw(2, 6, 186)).expect("build 186");

    assert_eq!(model_position(&low), [1.0, 2.0, 3.0]);
    assert_eq!(model_position(&high), [1.0, 2.0, 3.0]);
}

#[test]
fn rsw_rejects_unconsumed_trailing_bytes() {
    let mut bytes = rsw(2, 1, 0);
    bytes.extend_from_slice(&[0; 16]);

    let err = RoWorld::from_bytes(&bytes).expect_err("trailing bytes must fail");
    assert!(
        err.to_string().contains("unconsumed"),
        "unexpected error: {err}"
    );
}

// ------------------------------------------------------------------- GND

/// A 2x2 GND, optionally carrying a `split_w` x `split_h` water grid.
fn gnd(major: u8, minor: u8, water_levels: &[f32], split_w: u32, split_h: u32) -> Vec<u8> {
    let version = ((major as u16) << 8) | minor as u16;
    let (width, height) = (2u32, 2u32);
    let mut b = Buf::default();
    b.raw(b"GRGN").u8(major).u8(minor);
    b.u32(width).u32(height).f32(10.0);

    b.u32(1).u32(40).name("tex.bmp", 40); // textures
    b.i32(0).i32(1).i32(1).i32(1); // empty lightmap

    b.u32(1); // one tile
    b.f32s(&[0.0, 1.0, 0.0, 1.0]).f32s(&[0.0, 0.0, 1.0, 1.0]);
    b.u16(0).u16(0);
    if version >= 0x0107 {
        b.raw(&[255, 255, 255, 255]);
    }

    for _ in 0..width * height {
        b.f32s(&[1.0, 2.0, 3.0, 4.0]).i32(0).i32(-1).i32(-1);
    }

    if version >= 0x0108 {
        // Defaults, then the zone grid.
        b.f32(0.0).i32(1).f32(0.2).f32(2.0).f32(50.0).i32(3);
        b.u32(split_w).u32(split_h);
        for &level in water_levels {
            b.f32(level);
            if version >= 0x0109 {
                b.i32(2).f32(0.5).f32(1.0).f32(20.0).i32(4);
            }
        }
    }
    b.0
}

#[test]
fn gnd_1_7_has_no_water_block() {
    let ground = RoGround::from_bytes(&gnd(1, 7, &[], 0, 0)).expect("1.7 must parse");

    assert_eq!(ground.raw_version, 0x0107);
    assert!(ground.water.is_none());
}

#[test]
fn gnd_1_9_reads_a_split_water_grid() {
    // 2x2 zones with distinct levels: the grid is row-major.
    let ground = RoGround::from_bytes(&gnd(1, 9, &[1.0, 2.0, 3.0, 4.0], 2, 2)).expect("1.9");
    let water = ground.water.expect("1.9 carries water");

    assert_eq!((water.split_width, water.split_height), (2, 2));
    assert_eq!(
        water.zones.iter().map(|z| z.level).collect::<Vec<_>>(),
        vec![1.0, 2.0, 3.0, 4.0]
    );
    // Per-zone parameters are read at >= 1.9 rather than inherited.
    assert_eq!(water.zones[0].wave_height, 0.5);
}

#[test]
fn gnd_1_8_zones_inherit_everything_but_the_level() {
    let ground = RoGround::from_bytes(&gnd(1, 8, &[7.0], 1, 1)).expect("1.8");
    let water = ground.water.expect("1.8 carries water");

    assert_eq!(water.zones.len(), 1);
    assert_eq!(water.zones[0].level, 7.0);
    assert_eq!(
        water.zones[0].wave_height, 0.2,
        "inherited from the defaults"
    );
}

#[test]
fn gnd_water_zones_tile_the_map_evenly() {
    let ground = RoGround::from_bytes(&gnd(1, 9, &[1.0, 2.0, 3.0, 4.0], 2, 2)).expect("1.9");
    let water = ground.water.expect("water");

    // A 4x4 map over a 2x2 grid: cells 0-1 fall in zone column 0, cells 2-3 in 1.
    assert_eq!(water.zone_at(0, 0, 4, 4).level, 1.0);
    assert_eq!(water.zone_at(3, 0, 4, 4).level, 2.0);
    assert_eq!(water.zone_at(0, 3, 4, 4).level, 3.0);
    assert_eq!(water.zone_at(3, 3, 4, 4).level, 4.0);
}

#[test]
fn gnd_rejects_unconsumed_trailing_bytes() {
    let mut bytes = gnd(1, 7, &[], 0, 0);
    bytes.extend_from_slice(&[0; 4]);

    let err = RoGround::from_bytes(&bytes).expect_err("trailing bytes must fail");
    assert!(
        err.to_string().contains("unconsumed"),
        "unexpected error: {err}"
    );
}
