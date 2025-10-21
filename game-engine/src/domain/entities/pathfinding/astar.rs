use super::grid::PathfindingGrid;
use pathfinding::prelude::astar;

pub fn find_path(
    grid: &PathfindingGrid,
    start: (u16, u16),
    goal: (u16, u16),
) -> Option<Vec<(u16, u16)>> {
    if !grid.is_walkable(start.0, start.1) || !grid.is_walkable(goal.0, goal.1) {
        return None;
    }

    let result = astar(
        &start,
        |&(x, y)| successors(grid, x, y),
        |&(x, y)| heuristic((x, y), goal),
        |&pos| pos == goal,
    );

    result.map(|(path, _cost)| path)
}

fn successors(grid: &PathfindingGrid, x: u16, y: u16) -> Vec<((u16, u16), u32)> {
    let mut neighbors = Vec::with_capacity(8);

    let directions = [
        (0, -1, 10),
        (1, -1, 14),
        (1, 0, 10),
        (1, 1, 14),
        (0, 1, 10),
        (-1, 1, 14),
        (-1, 0, 10),
        (-1, -1, 14),
    ];

    for (dx, dy, cost) in directions {
        let nx = x as i32 + dx;
        let ny = y as i32 + dy;

        if nx < 0 || ny < 0 || nx >= grid.width() as i32 || ny >= grid.height() as i32 {
            continue;
        }

        let nx = nx as u16;
        let ny = ny as u16;

        if !grid.is_walkable(nx, ny) {
            continue;
        }

        if cost == 14 {
            let cx1 = (x as i32 + dx) as u16;
            let cy1 = y;
            let cx2 = x;
            let cy2 = (y as i32 + dy) as u16;

            if !grid.is_walkable(cx1, cy1) || !grid.is_walkable(cx2, cy2) {
                continue;
            }
        }

        neighbors.push(((nx, ny), cost));
    }

    neighbors
}

fn heuristic(pos: (u16, u16), goal: (u16, u16)) -> u32 {
    let dx = (pos.0 as i32 - goal.0 as i32).unsigned_abs();
    let dy = (pos.1 as i32 - goal.1 as i32).unsigned_abs();

    let diagonal = dx.min(dy);
    let straight = dx.max(dy) - diagonal;

    diagonal * 14 + straight * 10
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::ro_formats::gat::{GatCell, GatCellType, RoAltitude};

    fn create_test_grid(width: u32, height: u32, walkable_cells: &[(u16, u16)]) -> PathfindingGrid {
        let mut cells = vec![
            GatCell {
                height: [0.0; 4],
                cell_type: GatCellType::from(1),
            };
            (width * height) as usize
        ];

        for &(x, y) in walkable_cells {
            let index = (y as usize) * (width as usize) + (x as usize);
            cells[index].cell_type = GatCellType::from(0);
        }

        let gat = RoAltitude {
            version: "1.0".to_string(),
            width,
            height,
            cells,
        };

        PathfindingGrid::from_gat(&gat)
    }

    #[test]
    fn test_straight_path() {
        let grid = create_test_grid(10, 10, &[(0, 0), (1, 0), (2, 0), (3, 0), (4, 0)]);

        let path = find_path(&grid, (0, 0), (4, 0)).unwrap();
        assert_eq!(path.len(), 5);
        assert_eq!(path[0], (0, 0));
        assert_eq!(path[4], (4, 0));
    }

    #[test]
    fn test_diagonal_path() {
        let grid = create_test_grid(
            10,
            10,
            &[
                (0, 0),
                (1, 0),
                (0, 1),
                (1, 1),
                (2, 1),
                (1, 2),
                (2, 2),
                (3, 2),
                (2, 3),
                (3, 3),
            ],
        );

        let path = find_path(&grid, (0, 0), (3, 3)).unwrap();
        assert_eq!(path.len(), 4);
        assert_eq!(path[0], (0, 0));
        assert_eq!(path[3], (3, 3));
    }

    #[test]
    fn test_no_path() {
        let grid = create_test_grid(10, 10, &[(0, 0), (5, 5)]);
        let path = find_path(&grid, (0, 0), (5, 5));
        assert!(path.is_none());
    }

    #[test]
    fn test_unwalkable_start() {
        let grid = create_test_grid(10, 10, &[(1, 1)]);
        let path = find_path(&grid, (0, 0), (1, 1));
        assert!(path.is_none());
    }

    #[test]
    fn test_unwalkable_goal() {
        let grid = create_test_grid(10, 10, &[(0, 0)]);
        let path = find_path(&grid, (0, 0), (1, 1));
        assert!(path.is_none());
    }
}
