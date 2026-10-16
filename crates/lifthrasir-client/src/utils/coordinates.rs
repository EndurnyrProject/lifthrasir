/// Encode position and direction into 3 bytes
///
/// Used for entity spawn packets (STANDENTRY, NEWENTRY) to encode x, y coordinates
/// and facing direction in a compact format.
///
/// Encoding format:
/// - Byte 0: X[9:2]
/// - Byte 1: X[1:0] Y[9:4]
/// - Byte 2: Y[3:0] Dir[3:0]
///
/// # Arguments
///
/// * `x` - X coordinate (10-bit value, 0-1023)
/// * `y` - Y coordinate (10-bit value, 0-1023)
/// * `dir` - Direction (4-bit value, 0-15)
///
/// # Returns
///
/// 3-byte array containing the encoded position and direction
pub fn encode_pos_dir(x: u16, y: u16, dir: u8) -> [u8; 3] {
    let byte0 = (x >> 2) as u8;
    let byte1 = (((x << 6) | ((y >> 4) & 0x3F)) & 0xFF) as u8;
    let byte2 = (((y << 4) | ((dir as u16) & 0x0F)) & 0xFF) as u8;

    [byte0, byte1, byte2]
}

/// Decode position and direction from 3 bytes
///
/// Inverse of encode_pos_dir, extracts x, y coordinates and direction.
///
/// # Arguments
///
/// * `data` - 3-byte array containing encoded position and direction
///
/// # Returns
///
/// Tuple of (x, y, dir)
pub fn decode_pos_dir(data: [u8; 3]) -> (u16, u16, u8) {
    let x = ((data[0] as u16) << 2) | ((data[1] as u16) >> 6);
    let y = (((data[1] as u16) & 0x3F) << 4) | ((data[2] as u16) >> 4);
    let dir = data[2] & 0x0F;

    (x, y, dir)
}

/// Encode movement data into 6 bytes
///
/// Used for movement packets (MOVEENTRY) to encode source and destination coordinates.
///
/// Encoding format:
/// - byte0: x0[9:2]
/// - byte1: x0[1:0] y0[9:4]
/// - byte2: y0[3:0] x1[9:6]
/// - byte3: x1[5:0] y1[9:8]
/// - byte4: y1[7:0]
/// - byte5: sx[3:0] sy[3:0] (sub-cell offsets, typically 0)
///
/// # Arguments
///
/// * `src_x` - Source X coordinate (10-bit value, 0-1023)
/// * `src_y` - Source Y coordinate (10-bit value, 0-1023)
/// * `dst_x` - Destination X coordinate (10-bit value, 0-1023)
/// * `dst_y` - Destination Y coordinate (10-bit value, 0-1023)
///
/// # Returns
///
/// 6-byte array containing the encoded movement data
pub fn encode_move_data(src_x: u16, src_y: u16, dst_x: u16, dst_y: u16) -> [u8; 6] {
    let byte0 = (src_x >> 2) as u8;
    let byte1 = (((src_x << 6) | ((src_y >> 4) & 0x3F)) & 0xFF) as u8;
    let byte2 = (((src_y << 4) | ((dst_x >> 6) & 0x0F)) & 0xFF) as u8;
    let byte3 = (((dst_x << 2) | ((dst_y >> 8) & 0x03)) & 0xFF) as u8;
    let byte4 = (dst_y & 0xFF) as u8;
    let byte5 = 0; // Sub-cell offsets (typically 0)

    [byte0, byte1, byte2, byte3, byte4, byte5]
}

/// Decode movement data from 6 bytes
///
/// Inverse of encode_move_data, extracts source and destination coordinates.
///
/// # Arguments
///
/// * `data` - 6-byte array containing encoded movement data
///
/// # Returns
///
/// Tuple of (src_x, src_y, dst_x, dst_y)
pub fn decode_move_data(data: [u8; 6]) -> (u16, u16, u16, u16) {
    let src_x = ((data[0] as u16) << 2) | ((data[1] as u16) >> 6);
    let src_y = (((data[1] as u16) & 0x3F) << 4) | ((data[2] as u16) >> 4);
    let dst_x = (((data[2] as u16) & 0x0F) << 6) | ((data[3] as u16) >> 2);
    let dst_y = (((data[3] as u16) & 0x03) << 8) | (data[4] as u16);

    (src_x, src_y, dst_x, dst_y)
}

#[cfg(test)]
mod tests {
    use lifthrasir_runtime::rendering::Direction;

    #[test]
    fn ro_directions_convert_to_internal_facings() {
        let expected = [
            Direction::North,
            Direction::NorthWest,
            Direction::West,
            Direction::SouthWest,
            Direction::South,
            Direction::SouthEast,
            Direction::East,
            Direction::NorthEast,
        ];

        for (value, direction) in expected.into_iter().enumerate() {
            assert_eq!(Direction::from_ro_u8(value as u8), direction);
        }
    }
}
