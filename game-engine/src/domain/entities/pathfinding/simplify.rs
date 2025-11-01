/// Simplifies a path by removing unnecessary intermediate points
/// Uses Ramer-Douglas-Peucker algorithm with epsilon tolerance
pub fn simplify_path(points: &[(u16, u16)], epsilon: f32) -> Vec<(u16, u16)> {
    if points.len() < 3 {
        return points.to_vec();
    }

    let mut max_dist = 0.0;
    let mut max_index = 0;

    let start = (points[0].0 as f32, points[0].1 as f32);
    let end = (
        points[points.len() - 1].0 as f32,
        points[points.len() - 1].1 as f32,
    );

    for (i, &point_coords) in points.iter().enumerate().take(points.len() - 1).skip(1) {
        let point = (point_coords.0 as f32, point_coords.1 as f32);
        let dist = perpendicular_distance(point, start, end);
        if dist > max_dist {
            max_dist = dist;
            max_index = i;
        }
    }

    if max_dist > epsilon {
        let mut left = simplify_path(&points[..=max_index], epsilon);
        let right = simplify_path(&points[max_index..], epsilon);
        left.extend_from_slice(&right[1..]);
        left
    } else {
        vec![points[0], points[points.len() - 1]]
    }
}

fn perpendicular_distance(point: (f32, f32), line_start: (f32, f32), line_end: (f32, f32)) -> f32 {
    let (px, py) = point;
    let (x1, y1) = line_start;
    let (x2, y2) = line_end;

    let dx = x2 - x1;
    let dy = y2 - y1;

    if dx == 0.0 && dy == 0.0 {
        return ((px - x1).powi(2) + (py - y1).powi(2)).sqrt();
    }

    let t = ((px - x1) * dx + (py - y1) * dy) / (dx * dx + dy * dy);
    let t = t.clamp(0.0, 1.0);

    let closest_x = x1 + t * dx;
    let closest_y = y1 + t * dy;

    ((px - closest_x).powi(2) + (py - closest_y).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simplify_straight_line() {
        let points = vec![(0, 0), (1, 0), (2, 0), (3, 0), (4, 0), (5, 0)];
        let simplified = simplify_path(&points, 0.5);
        assert_eq!(simplified.len(), 2);
        assert_eq!(simplified[0], (0, 0));
        assert_eq!(simplified[1], (5, 0));
    }

    #[test]
    fn test_simplify_l_shape() {
        let points = vec![(0, 0), (1, 0), (2, 0), (2, 1), (2, 2)];
        let simplified = simplify_path(&points, 0.5);
        assert!(simplified.len() <= 3);
        assert_eq!(simplified[0], (0, 0));
        assert_eq!(*simplified.last().unwrap(), (2, 2));
    }

    #[test]
    fn test_simplify_too_short() {
        let points = vec![(0, 0), (1, 1)];
        let simplified = simplify_path(&points, 0.5);
        assert_eq!(simplified, points);
    }

    #[test]
    fn test_perpendicular_distance() {
        let point = (1.0, 1.0);
        let line_start = (0.0, 0.0);
        let line_end = (2.0, 0.0);

        let dist = perpendicular_distance(point, line_start, line_end);
        assert!((dist - 1.0).abs() < 0.01);
    }
}
