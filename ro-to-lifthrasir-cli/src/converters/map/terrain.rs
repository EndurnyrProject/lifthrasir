use anyhow::{Context, bail};
use glam::Vec3;
use ro_formats::{CELL_SIZE, GndSurface, GndTile, RoGround};
use std::collections::BTreeMap;

/// Terrain geometry batched per ground texture, ready to be written as one
/// glTF mesh primitive.
#[derive(Debug, Default, Clone)]
pub struct TerrainPrimitive {
    pub texture: String,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

type Batches = BTreeMap<usize, TerrainPrimitive>;

/// Build the ground geometry of a GND, one primitive per ground texture.
///
/// Ports the runtime terrain mesher: unshared per-cell quads, hard-normal wall
/// quads on the cells that declare them, smooth vertex normals averaged from
/// the neighbouring cell normals, and per-tile vertex colors as raw 0-1 floats.
pub fn build_terrain(ground: &RoGround) -> anyhow::Result<Vec<TerrainPrimitive>> {
    let width = ground.width as usize;
    let height = ground.height as usize;
    let expected = width * height;
    if ground.surfaces.len() != expected {
        bail!(
            "GND has {} surfaces, expected {expected} for {width}x{height}",
            ground.surfaces.len()
        );
    }

    let smooth_normals = calculate_smooth_normals(ground, width, height);
    let mut batches = Batches::new();

    for y in 0..height {
        for x in 0..width {
            let surface = &ground.surfaces[y * width + x];
            if surface.tile_up < 0 {
                continue;
            }
            let tile = tile_at(ground, surface.tile_up)?;
            let batch = batch_for(&mut batches, ground, tile)?;

            let h = &surface.height;
            let normals = &smooth_normals[y * width + x];
            let vertex_offset = batch.positions.len() as u32;

            batch.positions.push(cell_vertex(x, y, h[0], 0.0, 0.0));
            batch.positions.push(cell_vertex(x, y, h[1], 1.0, 0.0));
            batch.positions.push(cell_vertex(x, y, h[3], 1.0, 1.0));
            batch.positions.push(cell_vertex(x, y, h[3], 1.0, 1.0));
            batch.positions.push(cell_vertex(x, y, h[2], 0.0, 1.0));
            batch.positions.push(cell_vertex(x, y, h[0], 0.0, 0.0));

            batch.normals.push(normals[0].to_array());
            batch.normals.push(normals[1].to_array());
            batch.normals.push(normals[2].to_array());
            batch.normals.push(normals[2].to_array());
            batch.normals.push(normals[3].to_array());
            batch.normals.push(normals[0].to_array());

            let tile_color = gnd_tile_color_to_rgba(&tile.color);
            batch.colors.extend(std::iter::repeat_n(tile_color, 6));

            batch.uvs.push([tile.u1, tile.v1]);
            batch.uvs.push([tile.u2, tile.v2]);
            batch.uvs.push([tile.u4, tile.v4]);
            batch.uvs.push([tile.u4, tile.v4]);
            batch.uvs.push([tile.u3, tile.v3]);
            batch.uvs.push([tile.u1, tile.v1]);

            batch.indices.extend(vertex_offset..vertex_offset + 6);
        }
    }

    for y in 0..height {
        for x in 0..width {
            let surface = &ground.surfaces[y * width + x];

            if surface.tile_front >= 0 && (y + 1) < height {
                generate_wall(&mut batches, ground, x, y, surface, WallDirection::Front)?;
            }

            if surface.tile_right >= 0 && (x + 1) < width {
                generate_wall(&mut batches, ground, x, y, surface, WallDirection::Right)?;
            }
        }
    }

    Ok(batches.into_values().collect())
}

enum WallDirection {
    /// Wall on the cell's Y+1 boundary, facing negative Z.
    Front,
    /// Wall on the cell's X+1 boundary, facing negative X.
    Right,
}

/// Emit the quad bridging a cell to its neighbour, with hard normals and the
/// neighbour's raw GND heights (no smoothing).
fn generate_wall(
    batches: &mut Batches,
    ground: &RoGround,
    x: usize,
    y: usize,
    surface: &GndSurface,
    direction: WallDirection,
) -> anyhow::Result<()> {
    let width = ground.width as usize;
    let (tile_index, next_index) = match direction {
        WallDirection::Front => (surface.tile_front, (y + 1) * width + x),
        WallDirection::Right => (surface.tile_right, y * width + (x + 1)),
    };

    let tile = tile_at(ground, tile_index)?;
    let next = &ground.surfaces[next_index];
    let batch = batch_for(batches, ground, tile)?;
    let vertex_offset = batch.positions.len() as u32;

    match direction {
        WallDirection::Front => {
            let base_x = x as f32 * CELL_SIZE;
            let base_z = (y + 1) as f32 * CELL_SIZE;

            batch.positions.push([base_x, surface.height[2], base_z]);
            batch
                .positions
                .push([base_x + CELL_SIZE, surface.height[3], base_z]);
            batch.positions.push([base_x, next.height[0], base_z]);
            batch
                .positions
                .push([base_x + CELL_SIZE, next.height[1], base_z]);

            batch.uvs.push([tile.u1, tile.v1]);
            batch.uvs.push([tile.u2, tile.v2]);
            batch.uvs.push([tile.u3, tile.v3]);
            batch.uvs.push([tile.u4, tile.v4]);
        }
        WallDirection::Right => {
            let base_x = (x + 1) as f32 * CELL_SIZE;
            let base_z = y as f32 * CELL_SIZE;

            batch.positions.push([base_x, surface.height[1], base_z]);
            batch
                .positions
                .push([base_x, surface.height[3], base_z + CELL_SIZE]);
            batch.positions.push([base_x, next.height[0], base_z]);
            batch
                .positions
                .push([base_x, next.height[2], base_z + CELL_SIZE]);

            batch.uvs.push([tile.u2, tile.v2]);
            batch.uvs.push([tile.u1, tile.v1]);
            batch.uvs.push([tile.u4, tile.v4]);
            batch.uvs.push([tile.u3, tile.v3]);
        }
    }

    let normal = match direction {
        WallDirection::Front => [0.0, 0.0, -1.0],
        WallDirection::Right => [-1.0, 0.0, 0.0],
    };
    batch.normals.extend(std::iter::repeat_n(normal, 4));
    batch
        .colors
        .extend(std::iter::repeat_n(gnd_tile_color_to_rgba(&tile.color), 4));

    batch.indices.extend([
        vertex_offset,
        vertex_offset + 1,
        vertex_offset + 2,
        vertex_offset + 2,
        vertex_offset + 1,
        vertex_offset + 3,
    ]);

    Ok(())
}

/// Grid vertex position, with fractional offsets for the far corners of a cell.
fn cell_vertex(x: usize, y: usize, height: f32, offset_x: f32, offset_y: f32) -> [f32; 3] {
    [
        (x as f32 + offset_x) * CELL_SIZE,
        height,
        (y as f32 + offset_y) * CELL_SIZE,
    ]
}

/// GND stores the per-tile color as BGRA; the alpha byte is the real opacity.
fn gnd_tile_color_to_rgba(bgra: &[u8; 4]) -> [f32; 4] {
    [
        bgra[2] as f32 / 255.0,
        bgra[1] as f32 / 255.0,
        bgra[0] as f32 / 255.0,
        bgra[3] as f32 / 255.0,
    ]
}

fn tile_at(ground: &RoGround, index: i32) -> anyhow::Result<&GndTile> {
    ground.tiles.get(index as usize).with_context(|| {
        format!(
            "GND references tile {index}, only {} exist",
            ground.tiles.len()
        )
    })
}

/// Resolve the tile's texture slot to a ground texture and get its batch,
/// creating it on first use.
fn batch_for<'a>(
    batches: &'a mut Batches,
    ground: &RoGround,
    tile: &GndTile,
) -> anyhow::Result<&'a mut TerrainPrimitive> {
    let slot = tile.texture as usize;
    let index = *ground.texture_indexes.get(slot).with_context(|| {
        format!(
            "GND tile references texture slot {slot}, only {} exist",
            ground.texture_indexes.len()
        )
    })?;
    let texture = ground.textures.get(index).with_context(|| {
        format!(
            "GND texture slot {slot} resolves to texture {index}, only {} exist",
            ground.textures.len()
        )
    })?;

    Ok(batches.entry(index).or_insert_with(|| TerrainPrimitive {
        texture: texture.clone(),
        ..Default::default()
    }))
}

/// Base normal per cell, from the cross product of the quad's diagonals.
fn calculate_cell_normals(surfaces: &[GndSurface], width: usize, height: usize) -> Vec<Vec3> {
    let mut cell_normals = vec![Vec3::ZERO; width * height];

    for y in 0..height {
        for x in 0..width {
            let surface = &surfaces[y * width + x];
            if surface.tile_up < 0 {
                continue;
            }

            let sw = Vec3::from(cell_vertex(x, y, surface.height[0], 0.0, 0.0));
            let se = Vec3::from(cell_vertex(x, y, surface.height[1], 1.0, 0.0));
            let ne = Vec3::from(cell_vertex(x, y, surface.height[3], 1.0, 1.0));
            let nw = Vec3::from(cell_vertex(x, y, surface.height[2], 0.0, 1.0));

            cell_normals[y * width + x] = (ne - sw).cross(nw - se).normalize_or_zero();
        }
    }

    cell_normals
}

/// Smooth each cell's four vertex normals by averaging the cell normals that
/// touch that vertex.
fn calculate_smooth_normals(ground: &RoGround, width: usize, height: usize) -> Vec<[Vec3; 4]> {
    let cells = calculate_cell_normals(&ground.surfaces, width, height);
    let mut smooth = vec![[Vec3::ZERO; 4]; width * height];

    let average = |neighbours: [Option<usize>; 4]| {
        let (sum, count) = neighbours
            .into_iter()
            .flatten()
            .fold((Vec3::ZERO, 0), |(sum, count), idx| {
                (sum + cells[idx], count + 1)
            });
        (sum / count as f32).normalize_or_zero()
    };

    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            let left = (x > 0).then(|| y * width + (x - 1));
            let right = (x + 1 < width).then(|| y * width + (x + 1));
            let top = (y > 0).then(|| (y - 1) * width + x);
            let bottom = (y + 1 < height).then(|| (y + 1) * width + x);
            let top_left = (y > 0 && x > 0).then(|| (y - 1) * width + (x - 1));
            let top_right = (y > 0 && x + 1 < width).then(|| (y - 1) * width + (x + 1));
            let bottom_left = (y + 1 < height && x > 0).then(|| (y + 1) * width + (x - 1));
            let bottom_right = (y + 1 < height && x + 1 < width).then(|| (y + 1) * width + (x + 1));

            smooth[idx][0] = average([Some(idx), left, bottom_left, bottom]);
            smooth[idx][1] = average([Some(idx), right, bottom_right, bottom]);
            smooth[idx][2] = average([Some(idx), left, top_left, top]);
            smooth[idx][3] = average([Some(idx), right, top_right, top]);
        }
    }

    smooth
}

#[cfg(test)]
mod tests {
    use super::*;
    use ro_formats::{GndSurface, GndTile};

    fn tile(texture: u16) -> GndTile {
        GndTile {
            u1: 0.0,
            u2: 1.0,
            u3: 0.0,
            u4: 1.0,
            v1: 0.0,
            v2: 0.0,
            v3: 1.0,
            v4: 1.0,
            texture,
            color: [255, 255, 255, 255],
        }
    }

    fn flat_surface(tile_up: i32) -> GndSurface {
        GndSurface {
            height: [0.0; 4],
            tile_up,
            tile_front: -1,
            tile_right: -1,
        }
    }

    #[test]
    fn out_of_range_texture_slot_fails_loudly() {
        let ground = RoGround {
            version: "1.7".into(),
            width: 1,
            height: 1,
            textures: vec!["a.bmp".into()],
            texture_indexes: vec![0],
            tiles: vec![tile(7)],
            surfaces: vec![flat_surface(0)],
        };

        let err = build_terrain(&ground).expect_err("out of range slot must fail");
        assert!(
            err.to_string().contains("texture slot 7"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn steep_transition_emits_a_wall_quad_in_the_wall_tile_batch() {
        let ground = RoGround {
            version: "1.7".into(),
            width: 2,
            height: 2,
            textures: vec!["a.bmp".into(), "b.bmp".into()],
            texture_indexes: vec![0, 1],
            tiles: vec![tile(0), tile(1)],
            surfaces: vec![
                GndSurface {
                    height: [0.0; 4],
                    tile_up: 0,
                    tile_front: 1,
                    tile_right: -1,
                },
                flat_surface(0),
                GndSurface {
                    height: [-20.0; 4],
                    tile_up: 1,
                    tile_front: -1,
                    tile_right: -1,
                },
                GndSurface {
                    height: [-20.0; 4],
                    tile_up: 1,
                    tile_front: -1,
                    tile_right: -1,
                },
            ],
        };

        let primitives = build_terrain(&ground).expect("build");

        assert_eq!(primitives.len(), 2);
        assert_eq!(primitives[0].texture, "a.bmp");
        assert_eq!(primitives[0].positions.len(), 12);
        assert_eq!(primitives[0].indices.len(), 12);

        let walled = &primitives[1];
        assert_eq!(walled.texture, "b.bmp");
        assert_eq!(walled.positions.len(), 16);
        assert_eq!(walled.normals.len(), 16);
        assert_eq!(walled.colors.len(), 16);
        assert_eq!(walled.uvs.len(), 16);
        assert_eq!(walled.indices.len(), 18);

        assert_eq!(
            &walled.positions[12..],
            &[
                [0.0, 0.0, CELL_SIZE],
                [CELL_SIZE, 0.0, CELL_SIZE],
                [0.0, -20.0, CELL_SIZE],
                [CELL_SIZE, -20.0, CELL_SIZE],
            ]
        );
        assert!(walled.normals[12..].iter().all(|n| *n == [0.0, 0.0, -1.0]));
        assert_eq!(&walled.indices[12..], &[12, 13, 14, 14, 13, 15]);
    }

    #[test]
    fn flat_ground_has_uniform_up_normals() {
        let ground = RoGround {
            version: "1.7".into(),
            width: 2,
            height: 2,
            textures: vec!["a.bmp".into()],
            texture_indexes: vec![0],
            tiles: vec![tile(0)],
            surfaces: vec![
                flat_surface(0),
                flat_surface(0),
                flat_surface(0),
                flat_surface(0),
            ],
        };

        let primitives = build_terrain(&ground).expect("build");

        assert_eq!(primitives.len(), 1);
        let prim = &primitives[0];
        assert_eq!(prim.texture, "a.bmp");
        assert_eq!(prim.positions.len(), 24);
        assert!(
            prim.normals.iter().all(|n| *n == [0.0, -1.0, 0.0]),
            "expected every normal to point at world up (-Y), got {:?}",
            prim.normals
        );
    }
}
