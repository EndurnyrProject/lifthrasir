//! Reads the GND cell heights back out of the live terrain meshes, for the
//! tools that need the ground the converter saw (water tile selection, and the
//! terrain tools after it).

use super::layout::MapLayout;
use crate::EditorOnly;
use bevy::{mesh::VertexAttributeValues, prelude::*};
use lifthrasir_runtime::world::LifMapData;
use lifthrasir_tooling::converters::gltf_out::from_gltf_vec;
use ro_formats::CELL_SIZE;

/// Native GND corner heights (positive down) per cell, row-major, in GND
/// corner order `[(0,0), (1,0), (0,1), (1,1)]`. Cells without a ground quad
/// (holes) read `[0.0; 4]`.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainHeights {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<[f32; 4]>,
}

impl TerrainHeights {
    pub fn get(&self, x: usize, y: usize) -> [f32; 4] {
        self.cells[y * self.width + x]
    }
}

/// The heights of every ground quad of every chunk mesh under the terrain
/// node. Walls (quads without an XZ footprint) are skipped. The GND size is
/// half the GAT size.
pub fn terrain_heights(world: &World, layout: &MapLayout) -> TerrainHeights {
    let (width, height) = world
        .get::<LifMapData>(layout.root)
        .map(|data| {
            (
                data.altitude.width as usize / 2,
                data.altitude.height as usize / 2,
            )
        })
        .unwrap_or_default();
    let mut heights = TerrainHeights {
        width,
        height,
        cells: vec![[0.0; 4]; width * height],
    };
    let meshes = world.resource::<Assets<Mesh>>();
    let chunks = layout
        .terrain_node
        .and_then(|terrain| world.get::<Children>(terrain))
        .into_iter()
        .flat_map(|children| children.iter())
        .filter(|&chunk| world.get::<EditorOnly>(chunk).is_none())
        .filter_map(|chunk| meshes.get(&world.get::<Mesh3d>(chunk)?.0));
    for mesh in chunks {
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            continue;
        };
        for quad in positions.as_chunks::<4>().0 {
            let native = quad.map(|p| from_gltf_vec(Vec3::from(p)));
            write_ground_quad(&mut heights, native);
        }
    }
    heights
}

/// Stores a ground quad's heights in its cell; vertex order is
/// `[sw, se, ne, nw]`, so GND order is vertices `[0, 1, 3, 2]`.
fn write_ground_quad(heights: &mut TerrainHeights, [sw, se, ne, nw]: [Vec3; 4]) {
    let min = sw.min(se).min(ne).min(nw);
    let max = sw.max(se).max(ne).max(nw);
    if max.x <= min.x || max.z <= min.z {
        return;
    }
    let x = (min.x / CELL_SIZE).floor();
    let y = (min.z / CELL_SIZE).floor();
    if x < 0.0 || y < 0.0 || x as usize >= heights.width || y as usize >= heights.height {
        return;
    }
    heights.cells[y as usize * heights.width + x as usize] = [sw.y, se.y, nw.y, ne.y];
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty(width: usize, height: usize) -> TerrainHeights {
        TerrainHeights {
            width,
            height,
            cells: vec![[0.0; 4]; width * height],
        }
    }

    #[test]
    fn a_ground_quad_lands_in_its_cell_in_gnd_corner_order() {
        let mut heights = empty(3, 3);
        let (x, z) = (CELL_SIZE, 2.0 * CELL_SIZE);
        write_ground_quad(
            &mut heights,
            [
                Vec3::new(x, 1.0, z),
                Vec3::new(x + CELL_SIZE, 2.0, z),
                Vec3::new(x + CELL_SIZE, 4.0, z + CELL_SIZE),
                Vec3::new(x, 3.0, z + CELL_SIZE),
            ],
        );
        assert_eq!(heights.get(1, 2), [1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn walls_are_skipped() {
        let mut heights = empty(2, 2);
        let z = CELL_SIZE;
        write_ground_quad(
            &mut heights,
            [
                Vec3::new(0.0, 1.0, z),
                Vec3::new(CELL_SIZE, 1.0, z),
                Vec3::new(0.0, 5.0, z),
                Vec3::new(CELL_SIZE, 5.0, z),
            ],
        );
        assert_eq!(heights, empty(2, 2));
    }
}
