use bevy::prelude::*;

pub const SLOT_COUNT: usize = 8;

/// Calculates window-relative positions for a 4x2 grid of character slots.
///
/// The layout is defined relative to the window's height to ensure consistent
/// proportions across different 4:3 resolutions (800x600, 1024x768, 1440x1080, etc.).
/// Uses a center-based coordinate system where (0,0) is the screen center.
pub fn get_slot_positions(window_width: f32, window_height: f32) -> [Vec2; SLOT_COUNT] {
    const GRID_ROWS: usize = 2;
    const GRID_COLS: usize = 4;
    const TARGET_ASPECT_RATIO: f32 = 4.0 / 3.0;

    // Define layout geometry relative to window height to maintain shape
    const SLOT_HEIGHT_PERCENT: f32 = 0.18;
    const SLOT_GAP_VERTICAL_PERCENT: f32 = 0.08; // 8% of window height

    // Make slots proportional by relating width to height via aspect ratio
    const SLOT_WIDTH_PERCENT: f32 = SLOT_HEIGHT_PERCENT / TARGET_ASPECT_RATIO;
    const SLOT_GAP_HORIZONTAL_PERCENT: f32 = SLOT_GAP_VERTICAL_PERCENT / TARGET_ASPECT_RATIO;

    // Calculate total grid dimensions in normalized [0, 1] space
    let grid_width_norm = (GRID_COLS as f32 * SLOT_WIDTH_PERCENT)
        + ((GRID_COLS - 1) as f32 * SLOT_GAP_HORIZONTAL_PERCENT);
    let grid_height_norm = (GRID_ROWS as f32 * SLOT_HEIGHT_PERCENT)
        + ((GRID_ROWS - 1) as f32 * SLOT_GAP_VERTICAL_PERCENT);

    // Calculate top-left starting point to center the grid
    let start_x_norm = (1.0 - grid_width_norm) / 2.0;
    let start_y_norm = (1.0 - grid_height_norm) / 2.0;

    let mut positions = [Vec2::ZERO; SLOT_COUNT];
    for i in 0..SLOT_COUNT {
        let row = i / GRID_COLS;
        let col = i % GRID_COLS;

        // Calculate top-left of slot in normalized coordinates
        let x_norm =
            start_x_norm + (col as f32 * (SLOT_WIDTH_PERCENT + SLOT_GAP_HORIZONTAL_PERCENT));
        let y_norm =
            start_y_norm + (row as f32 * (SLOT_HEIGHT_PERCENT + SLOT_GAP_VERTICAL_PERCENT));

        // Calculate slot center
        let slot_center_x_norm = x_norm + (SLOT_WIDTH_PERCENT / 2.0);
        let slot_center_y_norm = y_norm + (SLOT_HEIGHT_PERCENT / 2.0);

        // Convert from normalized [0, 1] to center-based coordinates
        // (0,0) is at screen center, Y points up
        let screen_x = (slot_center_x_norm - 0.5) * window_width;
        let screen_y = (0.5 - slot_center_y_norm) * window_height; // Invert Y

        positions[i] = Vec2::new(screen_x, screen_y);
    }
    positions
}

/// Get world position for a character slot
/// Uses window-relative coordinates that adapt to different resolutions
pub fn slot_position(slot: u8, window: &Window) -> Vec3 {
    let positions = get_slot_positions(window.width(), window.height());
    let pos2d = positions[slot as usize];
    Vec3::new(pos2d.x, pos2d.y, 0.0)
}
