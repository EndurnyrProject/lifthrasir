use anyhow::{Context, ensure};
use lifthrasir_data::lif::{LifWater, LifWaterZone};
use ro_formats::{RoGround, RswWater};

pub fn resolve_water(ground: &RoGround, rsw: &RswWater) -> anyhow::Result<LifWater> {
    let Some(gnd) = ground.water.as_ref() else {
        return Ok(LifWater {
            split_width: 1,
            split_height: 1,
            zones: vec![LifWaterZone {
                level: rsw.level,
                water_type: rsw.water_type,
                wave_height: rsw.wave_height,
                wave_speed: rsw.wave_speed,
                wave_pitch: rsw.wave_pitch,
                anim_speed: rsw.anim_speed,
            }],
            width: ground.width,
            height: ground.height,
            buffer_view: 0,
        });
    };

    let zone_count = gnd
        .split_width
        .checked_mul(gnd.split_height)
        .context("GND water zone dimensions overflow")? as usize;
    ensure!(
        gnd.split_width != 0 && gnd.split_height != 0 && gnd.zones.len() == zone_count,
        "GND water declares a {}x{} zone grid but carries {} zones",
        gnd.split_width,
        gnd.split_height,
        gnd.zones.len()
    );

    let zones = gnd
        .zones
        .iter()
        .enumerate()
        .map(|(index, zone)| {
            Ok(LifWaterZone {
                level: zone.level,
                water_type: u32::try_from(zone.water_type).with_context(|| {
                    format!(
                        "GND water zone {index} has negative type {}",
                        zone.water_type
                    )
                })?,
                wave_height: zone.wave_height,
                wave_speed: zone.wave_speed,
                wave_pitch: zone.wave_pitch,
                anim_speed: u32::try_from(zone.anim_speed).with_context(|| {
                    format!(
                        "GND water zone {index} has negative animation speed {}",
                        zone.anim_speed
                    )
                })?,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(LifWater {
        split_width: gnd.split_width,
        split_height: gnd.split_height,
        zones,
        width: ground.width,
        height: ground.height,
        buffer_view: 0,
    })
}

pub fn select_water_tiles(ground: &RoGround, water: &LifWater) -> Vec<(usize, usize)> {
    let width = ground.width as usize;

    (0..ground.height as usize)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let zone = water.zone_at(x, y);
            let threshold = zone.level - zone.wave_height;
            let heights = ground.surfaces[y * width + x].height;
            heights[0].max(heights[1]).max(heights[2]).max(heights[3]) > threshold
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converters::map::fixtures::{mini_ground, mini_world};
    use ro_formats::{GndWater, GndWaterZone};

    #[test]
    fn selects_tiles_with_a_corner_above_the_strict_wave_threshold() {
        let mut ground = mini_ground();
        ground.surfaces[0].height = [8.0; 4];
        ground.surfaces[1].height = [7.0, 7.0, 7.0, 8.1];
        ground.surfaces[2].height = [9.0, 0.0, 0.0, 0.0];
        ground.surfaces[3].height = [7.0; 4];
        let mut rsw = mini_world().water;
        rsw.level = 10.0;
        rsw.wave_height = 2.0;
        let water = resolve_water(&ground, &rsw).expect("resolve water");

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
        let mut rsw = mini_world().water;
        rsw.level = 10.0;
        rsw.wave_height = 2.0;
        let water = resolve_water(&ground, &rsw).expect("resolve water");

        assert_eq!(select_water_tiles(&ground, &water), vec![(1, 0)]);
    }

    #[test]
    fn selects_each_tile_against_its_own_water_zone() {
        let mut ground = mini_ground();
        for surface in &mut ground.surfaces {
            surface.height = [9.0; 4];
        }
        ground.water = Some(GndWater {
            split_width: 2,
            split_height: 1,
            zones: vec![
                GndWaterZone {
                    level: 10.0,
                    water_type: 1,
                    wave_height: 2.0,
                    wave_speed: 1.0,
                    wave_pitch: 20.0,
                    anim_speed: 3,
                },
                GndWaterZone {
                    level: 12.0,
                    water_type: 2,
                    wave_height: 2.0,
                    wave_speed: 2.0,
                    wave_pitch: 30.0,
                    anim_speed: 4,
                },
            ],
        });
        let water = resolve_water(&ground, &mini_world().water).expect("resolve water");

        assert_eq!(select_water_tiles(&ground, &water), vec![(0, 0), (0, 1)]);
        assert_eq!(water.split_width, 2);
        assert_eq!(water.zones[1].water_type, 2);
    }
}
