use bevy::prelude::*;
use lifthrasir_runtime::world::terrain::{
    PathfindingGrid, find_path, raycast_terrain, spawn_coords_to_world_position,
    world_position_to_spawn_coords,
};
use ro_formats::{GatCell, GatCellType, RoAltitude};

fn flat(width: u32, height: u32) -> RoAltitude {
    RoAltitude {
        version: "1.0".into(),
        width,
        height,
        cells: vec![
            GatCell {
                height: [0.0; 4],
                cell_type: GatCellType::from(0)
            };
            (width * height) as usize
        ],
    }
}

#[test]
fn vertical_hit_preserves_height_and_corrected_walkability_cell() {
    let mut altitude = flat(8, 8);
    altitude.cells[4 * 8 + 2].cell_type = GatCellType::from(1);
    let position = spawn_coords_to_world_position(3, 3);
    let hit = raycast_terrain(
        &altitude,
        Ray3d::new(position + Vec3::Y * 10.0, Dir3::NEG_Y),
    )
    .unwrap();
    assert!((hit.world_position.y - 1.5).abs() < 0.01);
    assert_eq!(hit.cell_coords, (2, 4));
    assert!(!hit.is_walkable);
}

#[test]
fn shallow_ray_converges_without_plane_refinement() {
    let altitude = flat(64, 64);
    let ray = Ray3d::new(
        Vec3::new(5.0, 2.5, -50.0),
        Dir3::new(Vec3::new(1.0, -0.01, 0.0)).unwrap(),
    );
    let hit = raycast_terrain(&altitude, ray).unwrap();
    assert!((hit.world_position.x - 105.0).abs() < 0.01);
    assert!((hit.world_position.y - 1.5).abs() < 0.01);
    assert!(hit.is_walkable);
}

#[test]
fn horizontal_upward_and_off_map_rays_miss() {
    let altitude = flat(8, 8);
    for ray in [
        Ray3d::new(Vec3::new(5.0, 10.0, -5.0), Dir3::X),
        Ray3d::new(Vec3::new(5.0, 10.0, -5.0), Dir3::Y),
        Ray3d::new(Vec3::new(-10.0, 10.0, 10.0), Dir3::NEG_Y),
    ] {
        assert!(raycast_terrain(&altitude, ray).is_none());
    }
}

#[test]
fn path_queries_reject_corner_cutting_and_need_no_world_resources() {
    let mut altitude = flat(2, 2);
    altitude.cells[1].cell_type = GatCellType::from(1);
    altitude.cells[2].cell_type = GatCellType::from(1);
    assert!(find_path(&PathfindingGrid::from_gat(&altitude), (0, 0), (1, 1)).is_none());
    let altitude = flat(2, 2);
    assert_eq!(
        find_path(&PathfindingGrid::from_gat(&altitude), (0, 0), (1, 1)),
        Some(vec![(0, 0), (1, 1)])
    );
    for cell in [(0, 0), (1, 1), (500, 300)] {
        assert_eq!(
            world_position_to_spawn_coords(spawn_coords_to_world_position(cell.0, cell.1)),
            cell
        );
    }
}
