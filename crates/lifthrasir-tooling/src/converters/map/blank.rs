//! Brand-new map documents for the world editor, and the GAT height rule that
//! keeps walking on the terrain.

use crate::converters::map::document::{
    INDOOR_AMBIENT_BRIGHTNESS, INDOOR_EXPOSURE_EV100, MapDocument, MapLight,
    OUTDOOR_AMBIENT_BRIGHTNESS, OUTDOOR_EXPOSURE_EV100, TextureRef, look_to_rotation,
    point_light_candela, point_light_range, sun_direction, sun_illuminance,
};
use crate::converters::map::terrain::build_terrain;
use anyhow::Context;
use glam::Vec3;
use lifthrasir_data::lif;
use ro_formats::{
    GatCell, GatCellType, GndSurface, GndTile, RoAltitude, RoGround, RswLight, RswLightObj,
};
use std::path::PathBuf;

/// The GAT version new maps are written with.
pub const NEW_MAP_GAT_VERSION: &str = "1.2";

/// Which baked lighting and exposure preset a new map starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightingPreset {
    Outdoor,
    Indoor,
}

impl LightingPreset {
    pub fn is_indoor(self) -> bool {
        self == Self::Indoor
    }
}

/// Everything the new-map wizard asks for. `width`/`height` are GAT cells
/// (even, > 0); the GND is half that.
#[derive(Debug, Clone)]
pub struct NewMapParams {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub texture: TextureRef,
    /// Grayscale image whose luminance raises the terrain by up to
    /// `height_range`; `None` builds a flat map.
    pub heightmap: Option<PathBuf>,
    pub height_range: f32,
    pub preset: LightingPreset,
}

impl MapDocument {
    /// A valid, walkable single-texture map with a default sun and no objects.
    pub fn new_flat(params: &NewMapParams) -> anyhow::Result<MapDocument> {
        for (field, cells) in [("width", params.width), ("height", params.height)] {
            if cells == 0 || cells % 2 != 0 {
                anyhow::bail!("map {field} {cells} must be an even number of GAT cells above 0");
            }
        }
        let (gnd_width, gnd_height) = (params.width / 2, params.height / 2);
        let corners = match &params.heightmap {
            Some(path) => heightmap_corners(path, gnd_width, gnd_height, params.height_range)?,
            None => vec![0.0; ((gnd_width + 1) * (gnd_height + 1)) as usize],
        };
        let ground = blank_ground(params, gnd_width, gnd_height, &corners);
        let indoor = params.preset.is_indoor();
        let light = RswLight::default();

        let doc = MapDocument {
            name: params.name.clone(),
            meta: lif::LifMap {
                format_version: lif::MAP_FORMAT_VERSION,
                source: None,
                ambient_color: light.ambient,
                no_shade_tint: lif::no_shade_tint(light.ambient, light.diffuse),
                indoor,
                ambient_brightness: if indoor {
                    INDOOR_AMBIENT_BRIGHTNESS
                } else {
                    OUTDOOR_AMBIENT_BRIGHTNESS
                },
                exposure_ev100: if indoor {
                    INDOOR_EXPOSURE_EV100
                } else {
                    OUTDOOR_EXPOSURE_EV100
                },
            },
            textures: vec![params.texture.clone()],
            terrain: build_terrain(&ground)?,
            lights: vec![MapLight::Sun {
                rotation: look_to_rotation(sun_direction(&light), Vec3::NEG_Y),
                color: light.diffuse,
                lux: sun_illuminance(&light, indoor),
            }],
            props: Vec::new(),
            sounds: Vec::new(),
            effects: Vec::new(),
            water: None,
            gat: gat_from_ground(&ground),
        };
        doc.validate()
            .map_err(|errors| anyhow::anyhow!(errors.join("; ")))?;
        Ok(doc)
    }
}

/// Starting values for an editor-placed point or spot light.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightDefaults {
    pub color: [f32; 3],
    pub candela: f32,
    pub range: f32,
    pub inner_angle: f32,
    pub outer_angle: f32,
}

/// A white range-40 RSW light through the converter's own candela and range
/// rules, so placed lights match converted ones on the same map.
pub fn default_light(indoor: bool) -> LightDefaults {
    let light = RswLightObj {
        name: String::new(),
        position: [0.0; 3],
        color: [1.0; 3],
        range: 40.0,
    };
    LightDefaults {
        color: light.color,
        candela: point_light_candela(&light, indoor),
        range: point_light_range(&light, indoor),
        inner_angle: 0.3,
        outer_angle: 0.6,
    }
}

/// Native GND corner heights `-luma * height_range`, row-major over
/// `(gnd_width + 1) x (gnd_height + 1)` corners. Corner `(cx, cy)` samples the
/// image bilinearly at `u = cx / gnd_width`, `v = 1 - cy / gnd_height` with the
/// edges on the outer pixel centres; image row 0 is the north edge
/// (`y = gnd_height`).
fn heightmap_corners(
    path: &std::path::Path,
    gnd_width: u32,
    gnd_height: u32,
    height_range: f32,
) -> anyhow::Result<Vec<f32>> {
    let image = image::open(path)
        .with_context(|| format!("opening heightmap {}", path.display()))?
        .to_luma32f();
    let (image_width, image_height) = image.dimensions();
    let luma = |x: u32, y: u32| image.get_pixel(x, y).0[0];
    let sample = |u: f32, v: f32| {
        let x = u * (image_width - 1) as f32;
        let y = v * (image_height - 1) as f32;
        let (x0, y0) = (x.floor() as u32, y.floor() as u32);
        let (x1, y1) = (
            (x0 + 1).min(image_width - 1),
            (y0 + 1).min(image_height - 1),
        );
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let top = luma(x0, y0) * (1.0 - fx) + luma(x1, y0) * fx;
        let bottom = luma(x0, y1) * (1.0 - fx) + luma(x1, y1) * fx;
        top * (1.0 - fy) + bottom * fy
    };
    Ok((0..=gnd_height)
        .flat_map(|cy| (0..=gnd_width).map(move |cx| (cx, cy)))
        .map(|(cx, cy)| {
            let u = cx as f32 / gnd_width as f32;
            let v = 1.0 - cy as f32 / gnd_height as f32;
            -sample(u, v) * height_range
        })
        .collect())
}

/// A one-texture GND whose corner heights come from the row-major
/// `(gnd_width + 1) x (gnd_height + 1)` grid `corners`, so neighbouring cells
/// share their corner samples.
fn blank_ground(
    params: &NewMapParams,
    gnd_width: u32,
    gnd_height: u32,
    corners: &[f32],
) -> RoGround {
    let stride = (gnd_width + 1) as usize;
    let corner = |x: u32, y: u32| corners[y as usize * stride + x as usize];
    let surfaces = (0..gnd_height)
        .flat_map(|y| (0..gnd_width).map(move |x| (x, y)))
        .map(|(x, y)| GndSurface {
            height: [
                corner(x, y),
                corner(x + 1, y),
                corner(x, y + 1),
                corner(x + 1, y + 1),
            ],
            tile_up: 0,
            tile_front: -1,
            tile_right: -1,
        })
        .collect();
    RoGround {
        version: "1.7".into(),
        raw_version: 0x0107,
        width: gnd_width,
        height: gnd_height,
        textures: vec![params.texture.name.clone()],
        texture_indexes: vec![0],
        tiles: vec![GndTile {
            u1: 0.0,
            v1: 0.0,
            u2: 1.0,
            v2: 0.0,
            u3: 0.0,
            v3: 1.0,
            u4: 1.0,
            v4: 1.0,
            texture: 0,
            color: [255; 4],
        }],
        surfaces,
        water: None,
    }
}

/// An all-walkable (raw type 0) GAT of `2 * ground.width x 2 * ground.height`
/// cells whose heights follow the GND with [`gat_cell_heights`].
pub fn gat_from_ground(ground: &RoGround) -> RoAltitude {
    let (width, height) = (ground.width * 2, ground.height * 2);
    let cells = (0..height as usize)
        .flat_map(|y| (0..width as usize).map(move |x| (x, y)))
        .map(|(x, y)| {
            let surface = &ground.surfaces[(y / 2) * ground.width as usize + x / 2];
            GatCell {
                height: gat_cell_heights(surface.height, x % 2, y % 2),
                cell_type: GatCellType::from(0),
            }
        })
        .collect();
    RoAltitude {
        version: NEW_MAP_GAT_VERSION.into(),
        width,
        height,
        cells,
    }
}

/// Native heights of GAT sub-cell `(sub_x, sub_y)` (each 0 or 1) of a GND cell
/// whose corners are in `GndSurface.height` order `[(0,0), (1,0), (0,1), (1,1)]`,
/// returned in the same order.
pub fn gat_cell_heights(gnd_corners: [f32; 4], sub_x: usize, sub_y: usize) -> [f32; 4] {
    let [h00, h10, h01, h11] = gnd_corners;
    [(0, 0), (1, 0), (0, 1), (1, 1)].map(|(du, dv)| {
        let u = (sub_x + du) as f32 / 2.0;
        let v = (sub_y + dv) as f32 / 2.0;
        h00 * (1.0 - u) * (1.0 - v) + h10 * u * (1.0 - v) + h01 * (1.0 - u) * v + h11 * u * v
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converters::map::document::{OUTDOOR_AMBIENT_BRIGHTNESS, OUTDOOR_EXPOSURE_EV100};
    use ro_formats::CELL_SIZE;

    fn grass() -> TextureRef {
        TextureRef {
            name: "grass01".into(),
            uri: "ro://ground/tex/grass01.ktx2".into(),
        }
    }

    fn params(name: &str, width: u32, height: u32) -> NewMapParams {
        NewMapParams {
            name: name.into(),
            width,
            height,
            texture: grass(),
            heightmap: None,
            height_range: 50.0,
            preset: LightingPreset::Outdoor,
        }
    }

    #[test]
    fn a_flat_outdoor_map_is_valid_and_fully_walkable() {
        let doc = MapDocument::new_flat(&params("new_map", 40, 34)).unwrap();

        let quads: usize = doc.terrain.iter().map(|p| p.positions.len() / 4).sum();
        let max_x = doc.terrain.iter().flat_map(|p| &p.positions).map(|p| p[0]);
        let max_z = doc.terrain.iter().flat_map(|p| &p.positions).map(|p| p[2]);
        assert_eq!(quads, 20 * 17);
        assert_eq!(max_x.fold(0.0, f32::max), 20.0 * CELL_SIZE);
        assert_eq!(max_z.fold(0.0, f32::max), 17.0 * CELL_SIZE);

        assert_eq!((doc.gat.width, doc.gat.height), (40, 34));
        assert_eq!(doc.gat.cells.len(), 40 * 34);
        assert!(doc.gat.cells.iter().all(|cell| cell.cell_type.raw() == 0
            && cell.cell_type.is_walkable()
            && cell.height == [0.0; 4]));

        assert_eq!(doc.textures, vec![grass()]);
        assert_eq!(doc.meta.source, None);
        assert!(!doc.meta.indoor);
        assert_eq!(doc.meta.exposure_ev100, OUTDOOR_EXPOSURE_EV100);
        assert_eq!(doc.meta.ambient_brightness, OUTDOOR_AMBIENT_BRIGHTNESS);
        assert!(doc.water.is_none() && doc.props.is_empty());
        assert_eq!(doc.validate(), Ok(()));
        assert!(doc.gat.to_bytes().is_ok());
    }

    #[test]
    fn a_heightmap_raises_the_terrain_north_up_and_the_gat_follows() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("height.png");
        // Row 0 is the north edge: north-west 0, north-east 51, south-west
        // 102, south-east 255.
        image::GrayImage::from_raw(2, 2, vec![0, 51, 102, 255])
            .unwrap()
            .save(&path)
            .unwrap();
        let mut params = params("hills", 2, 2);
        params.heightmap = Some(path);

        let doc = MapDocument::new_flat(&params).unwrap();

        // One quad, vertices [sw, se, ne, nw]; native heights are -luma * 50.
        let heights: Vec<f32> = doc.terrain[0].positions.iter().map(|p| p[1]).collect();
        let expected = [-20.0, -50.0, -10.0, 0.0];
        assert!(
            heights
                .iter()
                .zip(expected)
                .all(|(h, e)| (h - e).abs() < 1e-4),
            "{heights:?}"
        );
        // GAT sub-cell (1, 1): centre, east-edge midpoint, north-edge midpoint, north-east.
        let gat = doc.gat.cells[3].height;
        let expected = [-20.0, -30.0, -5.0, -10.0];
        assert!(
            gat.iter().zip(expected).all(|(h, e)| (h - e).abs() < 1e-4),
            "{gat:?}"
        );
    }

    #[test]
    fn invalid_sizes_and_names_are_rejected_naming_the_field() {
        for (params, field) in [
            (params("new_map", 41, 34), "width"),
            (params("new_map", 40, 0), "height"),
            (params("Bad Name", 40, 34), "name"),
        ] {
            let error = MapDocument::new_flat(&params).unwrap_err().to_string();
            assert!(error.contains(field), "{field}: {error}");
        }
    }

    /// Rebuilds each GND cell's native corner heights from the terrain ground
    /// quads of a locally converted retail map and checks the embedded GAT
    /// follows them with [`gat_cell_heights`].
    #[test]
    #[ignore = "requires a locally converted assets/data/maps/prontera/prontera.glb"]
    fn retail_gat_heights_follow_the_terrain() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/data/maps/prontera/prontera.glb");
        let gltf::Gltf { document, blob } = gltf::Gltf::open(&path).expect("open prontera");
        let blob = blob.expect("BIN chunk");
        let terrain = document
            .nodes()
            .find(|node| node.name() == Some("terrain"))
            .and_then(|node| node.mesh())
            .expect("terrain mesh");

        let mut gnd = std::collections::HashMap::new();
        for primitive in terrain.primitives() {
            let positions: Vec<[f32; 3]> = primitive
                .reader(|_| Some(&blob))
                .read_positions()
                .expect("positions")
                .collect();
            for quad in positions.as_chunks::<4>().0 {
                let xs = quad.iter().map(|p| p[0]);
                let zs = quad.iter().map(|p| -p[2]);
                let (min_x, max_x) = (
                    xs.clone().fold(f32::MAX, f32::min),
                    xs.fold(f32::MIN, f32::max),
                );
                let (min_z, max_z) = (
                    zs.clone().fold(f32::MAX, f32::min),
                    zs.fold(f32::MIN, f32::max),
                );
                if max_x - min_x < 1e-3 || max_z - min_z < 1e-3 {
                    continue;
                }
                let cell = (
                    (min_x / CELL_SIZE).floor() as usize,
                    (min_z / CELL_SIZE).floor() as usize,
                );
                let height = |vertex: usize| -quad[vertex][1];
                gnd.insert(cell, [height(0), height(1), height(3), height(2)]);
            }
        }

        let root = document.into_json();
        let gat: lif::LifGat =
            crate::converters::gltf_out::root_extension(&root, lif::EXTENSION_GAT)
                .expect("LIF_gat");
        let bytes =
            crate::converters::map::validate::gat_bytes(&root, &blob, &gat).expect("gat bytes");
        let altitude = RoAltitude::from_bytes(bytes).expect("parse GAT");

        let (mut matched, mut total) = (0usize, 0usize);
        for y in 0..altitude.height as usize {
            for x in 0..altitude.width as usize {
                let Some(corners) = gnd.get(&(x / 2, y / 2)) else {
                    continue;
                };
                let expected = gat_cell_heights(*corners, x % 2, y % 2);
                let actual = altitude.get_cell(x, y).expect("GAT cell").height;
                total += 4;
                matched += expected
                    .iter()
                    .zip(actual)
                    .filter(|(e, a)| (*e - a).abs() < 0.01)
                    .count();
            }
        }
        let ratio = matched as f64 / total as f64;
        assert!(total > 0, "no GAT corner sits on a ground quad");
        assert!(
            ratio >= 0.99,
            "only {matched}/{total} GAT corners ({ratio:.4}) follow the terrain"
        );
    }

    #[test]
    fn gat_heights_interpolate_a_sloped_gnd_cell() {
        let gnd = [0.0, 4.0, 8.0, 20.0];

        assert_eq!(gat_cell_heights(gnd, 0, 0), [0.0, 2.0, 4.0, 8.0]);
        assert_eq!(gat_cell_heights(gnd, 1, 0), [2.0, 4.0, 8.0, 12.0]);
        assert_eq!(gat_cell_heights(gnd, 0, 1), [4.0, 8.0, 8.0, 14.0]);
        assert_eq!(gat_cell_heights(gnd, 1, 1), [8.0, 12.0, 14.0, 20.0]);
    }
}
