/// Apply magenta transparency to RGBA image data
/// In Ragnarok Online, magenta (255, 0, 255) is treated as transparent
/// Uses tolerance to catch near-magenta colors from filtering/compression
pub fn apply_magenta_transparency(rgba_data: &mut [u8]) {
    // Thresholds for near-magenta detection
    // Broad thresholds to catch variations from texture filtering/compression
    const MAGENTA_THRESHOLD: u8 = 240; // R and B should be >= 240 (was 250)
    const GREEN_THRESHOLD: u8 = 15; // G should be <= 15 (was 5)

    // Process every 4 bytes (RGBA)
    for pixel in rgba_data.as_chunks_mut::<4>().0 {
        // Check if this pixel is close to magenta
        // R >= 240, G <= 15, B >= 240
        let is_near_magenta = pixel[0] >= MAGENTA_THRESHOLD
            && pixel[1] <= GREEN_THRESHOLD
            && pixel[2] >= MAGENTA_THRESHOLD;

        if is_near_magenta {
            // Zero out RGB and alpha to make it fully transparent
            // Setting RGB to 0 prevents color bleeding in rendering
            pixel[0] = 0; // R
            pixel[1] = 0; // G
            pixel[2] = 0; // B
            pixel[3] = 0; // A
        }
    }
}
