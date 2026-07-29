use ro_formats::{RoGround, RswWater};

pub fn select_water_tiles(ground: &RoGround, water: &RswWater) -> Vec<(usize, usize)> {
    let width = ground.width as usize;
    let threshold = water.level - water.wave_height;

    (0..ground.height as usize)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let heights = ground.surfaces[y * width + x].height;
            heights[0].max(heights[1]).max(heights[2]).max(heights[3]) > threshold
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converters::map::fixtures::{mini_ground, mini_world};

    #[test]
    fn selects_tiles_with_a_corner_above_the_strict_wave_threshold() {
        let mut ground = mini_ground();
        ground.surfaces[0].height = [8.0; 4];
        ground.surfaces[1].height = [7.0, 7.0, 7.0, 8.1];
        ground.surfaces[2].height = [9.0, 0.0, 0.0, 0.0];
        ground.surfaces[3].height = [7.0; 4];
        let mut water = mini_world().water;
        water.level = 10.0;
        water.wave_height = 2.0;

        assert_eq!(select_water_tiles(&ground, &water), vec![(1, 0), (0, 1)]);
    }

    /// The native renderer compared with a strict `>`, so a tile whose top sits
    /// exactly on `level - wave_height` stayed dry. This is the parity the
    /// converter has to preserve: once the native path is deleted there is no
    /// reference left to rediscover it from.
    #[test]
    fn excludes_a_tile_whose_top_exactly_equals_the_threshold() {
        let mut ground = mini_ground();
        ground.surfaces[0].height = [8.0; 4];
        ground.surfaces[1].height = [8.0, 8.0, 8.0, 8.5];
        ground.surfaces[2].height = [7.9; 4];
        ground.surfaces[3].height = [7.0; 4];
        let mut water = mini_world().water;
        water.level = 10.0;
        water.wave_height = 2.0;

        assert_eq!(select_water_tiles(&ground, &water), vec![(1, 0)]);
    }
}
