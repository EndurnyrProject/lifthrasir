use serde::{Deserialize, Serialize};

/// Player position and direction in the game world
///
/// RO uses a compressed 3-byte format for position data:
/// - X: 10-bit unsigned integer (0-1023)
/// - Y: 10-bit unsigned integer (0-1023)
/// - Direction: 4-bit unsigned integer (0-15)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Position {
    pub x: u16,
    pub y: u16,
    pub dir: u8,
}

impl Position {
    /// Create a new position with coordinates and direction
    pub fn new(x: u16, y: u16, dir: u8) -> Self {
        Self { x, y, dir }
    }

    /// Encode position and direction into 3 bytes
    ///
    /// Format: X and Y are 10-bit values, direction is 4-bit
    /// ```text
    /// Byte 0: X[9:2]
    /// Byte 1: X[1:0] Y[9:4]
    /// Byte 2: Y[3:0] Dir[3:0]
    /// ```
    pub fn encode(&self) -> [u8; 3] {
        let byte0 = (self.x >> 2) as u8;
        let byte1 = (((self.x << 6) | ((self.y >> 4) & 0x3F)) & 0xFF) as u8;
        let byte2 = (((self.y << 4) | (self.dir as u16 & 0x0F)) & 0xFF) as u8;
        [byte0, byte1, byte2]
    }

    /// Decode position and direction from 3 bytes
    pub fn decode(data: [u8; 3]) -> Self {
        let x = ((data[0] as u16) << 2) | ((data[1] as u16) >> 6);
        let y = (((data[1] as u16) & 0x3F) << 4) | ((data[2] as u16) >> 4);
        let dir = data[2] & 0x0F;
        Self { x, y, dir }
    }
}

/// Spawn data sent when a player enters the game world
///
/// Contains initial position, server tick for synchronization,
/// and character size information for collision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnData {
    /// Server tick for time synchronization
    pub server_tick: u32,

    /// Player's spawn position and facing direction
    pub position: Position,

    /// Character's X size (for collision detection)
    pub x_size: u8,

    /// Character's Y size (for collision detection)
    pub y_size: u8,

    /// Font ID for character name display
    pub font: u16,
}

impl SpawnData {
    /// Create new spawn data
    pub fn new(
        server_tick: u32,
        position: Position,
        x_size: u8,
        y_size: u8,
        font: u16,
    ) -> Self {
        Self {
            server_tick,
            position,
            x_size,
            y_size,
            font,
        }
    }
}

/// Zone entry refusal error codes
///
/// Sent by the zone server when entry is denied.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ZoneEntryError {
    /// Normal (no error) - shouldn't happen in refuse packet
    Normal,
    /// Server closed
    ServerClosed,
    /// Someone has already logged in with this ID
    AlreadyLoggedIn,
    /// Already logged in (duplicate state)
    AlreadyLoggedInAlt,
    /// Environment error
    EnvironmentError,
    /// Server still recognizes last connection
    PreviousConnectionActive,
    /// Unknown error code
    Unknown(u8),
}

impl From<u8> for ZoneEntryError {
    fn from(value: u8) -> Self {
        match value {
            0 => ZoneEntryError::Normal,
            1 => ZoneEntryError::ServerClosed,
            2 => ZoneEntryError::AlreadyLoggedIn,
            3 => ZoneEntryError::AlreadyLoggedInAlt,
            4 => ZoneEntryError::EnvironmentError,
            8 => ZoneEntryError::PreviousConnectionActive,
            other => ZoneEntryError::Unknown(other),
        }
    }
}

impl ZoneEntryError {
    /// Get human-readable description of the error
    pub fn description(&self) -> &'static str {
        match self {
            ZoneEntryError::Normal => "Normal (no error)",
            ZoneEntryError::ServerClosed => "Server closed",
            ZoneEntryError::AlreadyLoggedIn => "Someone has already logged in with this ID",
            ZoneEntryError::AlreadyLoggedInAlt => "Already logged in",
            ZoneEntryError::EnvironmentError => "Environment error",
            ZoneEntryError::PreviousConnectionActive => {
                "Server still recognizes last connection"
            }
            ZoneEntryError::Unknown(_) => "Unknown error",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_encoding_decoding() {
        let test_cases = vec![
            (100, 200, 3),
            (0, 0, 0),
            (1023, 1023, 15), // Max values (10-bit for x/y, 4-bit for dir)
            (512, 512, 7),
            (1, 1, 1),
        ];

        for (x, y, dir) in test_cases {
            let position = Position::new(x, y, dir);
            let encoded = position.encode();
            let decoded = Position::decode(encoded);

            assert_eq!(position.x, decoded.x, "X coordinate mismatch for ({}, {}, {})", x, y, dir);
            assert_eq!(position.y, decoded.y, "Y coordinate mismatch for ({}, {}, {})", x, y, dir);
            assert_eq!(position.dir, decoded.dir, "Direction mismatch for ({}, {}, {})", x, y, dir);
        }
    }

    #[test]
    fn test_zone_entry_error_conversion() {
        assert_eq!(ZoneEntryError::from(0), ZoneEntryError::Normal);
        assert_eq!(ZoneEntryError::from(1), ZoneEntryError::ServerClosed);
        assert_eq!(ZoneEntryError::from(2), ZoneEntryError::AlreadyLoggedIn);
        assert_eq!(ZoneEntryError::from(99), ZoneEntryError::Unknown(99));
    }

    #[test]
    fn test_zone_entry_error_description() {
        assert_eq!(ZoneEntryError::ServerClosed.description(), "Server closed");
        assert_eq!(
            ZoneEntryError::AlreadyLoggedIn.description(),
            "Someone has already logged in with this ID"
        );
    }
}
