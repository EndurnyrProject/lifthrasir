use bevy::prelude::*;

/// Event fired when user requests movement (e.g., by clicking terrain)
///
/// This event represents the user's intent to move. It will be consumed
/// by the networking layer to send a CZ_REQUEST_MOVE2 packet to the server.
#[derive(Message, Debug, Clone)]
pub struct MovementRequested {
    /// Entity requesting movement (usually the player character)
    pub entity: Entity,

    /// Target X coordinate in RO coordinates (0-1023)
    pub dest_x: u16,

    /// Target Y coordinate in RO coordinates (0-1023)
    pub dest_y: u16,

    /// Desired facing direction after movement (0-15)
    pub direction: u8,
}

/// Event fired when server confirms movement
///
/// This event is generated from the ZC_NOTIFY_PLAYERMOVE packet.
/// It contains the server-authoritative movement data including
/// source and destination positions for interpolation.
#[derive(Message, Debug, Clone)]
pub struct MovementConfirmed {
    /// Entity that should move (usually the player character)
    pub entity: Entity,

    /// Server-confirmed source position X
    pub src_x: u16,

    /// Server-confirmed source position Y
    pub src_y: u16,

    /// Server-confirmed destination position X
    pub dest_x: u16,

    /// Server-confirmed destination position Y
    pub dest_y: u16,

    /// Server tick for synchronization
    pub server_tick: u32,
}

/// Event fired when movement completes or is interrupted
///
/// This can be triggered by:
/// - Movement completing naturally (reached destination)
/// - Server forcing stop (ZC_NOTIFY_MOVE_STOP packet)
/// - Client-side interruption (e.g., new movement request)
#[derive(Message, Debug, Clone)]
pub struct MovementStopped {
    /// Entity that stopped moving
    pub entity: Entity,

    /// Final X position (RO coordinates)
    pub x: u16,

    /// Final Y position (RO coordinates)
    pub y: u16,

    /// Reason for stopping
    pub reason: StopReason,
}

/// Reason why movement stopped
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// Movement completed successfully
    ReachedDestination,

    /// Server forced stop
    ServerInterrupted,

    /// Client interrupted (new movement request)
    ClientInterrupted,

    /// Blocked by obstacle or game state
    Blocked,
}
