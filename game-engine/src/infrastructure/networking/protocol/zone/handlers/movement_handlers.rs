use crate::infrastructure::networking::{
    errors::NetworkError,
    protocol::{
        traits::{EventWriter, PacketHandler},
        zone::{
            protocol::{ZoneContext, ZoneProtocol},
            server_packets::{ZcNotifyMoveStopPacket, ZcNotifyPlayermovePacket},
        },
    },
};
use bevy::prelude::*;

/// Event emitted when server confirms player movement
#[derive(Message, Debug, Clone)]
pub struct MovementConfirmedByServer {
    pub src_x: u16,
    pub src_y: u16,
    pub dest_x: u16,
    pub dest_y: u16,
    pub server_tick: u32,
}

/// Event emitted when server forces movement to stop
///
/// TODO: Multi-Character Support
/// Add `account_id: u32` field to identify which character stopped.
/// The ZC_NOTIFY_MOVE_STOP packet includes account_id but it's currently
/// not passed through. When CharacterRegistry is implemented, this will
/// enable stopping movement for any character, not just the local player.
#[derive(Message, Debug, Clone)]
pub struct MovementStoppedByServer {
    pub x: u16,
    pub y: u16,
    pub server_tick: u32,
    // TODO: Add account_id field for multi-character support
}

/// Handler for ZC_NOTIFY_PLAYERMOVE packet
///
/// Processes server confirmation of player movement and emits an event
/// with movement data for the client-side interpolation system.
pub struct PlayermoveHandler;

impl PacketHandler<ZoneProtocol> for PlayermoveHandler {
    type Packet = ZcNotifyPlayermovePacket;

    fn handle(
        &self,
        packet: Self::Packet,
        context: &mut ZoneContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        debug!(
            "Movement confirmed by server: ({}, {}) -> ({}, {}) at tick {}",
            packet.src_x, packet.src_y, packet.dest_x, packet.dest_y, packet.server_tick
        );

        // Update server tick for synchronization
        context.server_tick = packet.server_tick;

        // Emit event for movement system
        let event = MovementConfirmedByServer {
            src_x: packet.src_x,
            src_y: packet.src_y,
            dest_x: packet.dest_x,
            dest_y: packet.dest_y,
            server_tick: packet.server_tick,
        };

        event_writer.send_event(Box::new(event));

        Ok(())
    }
}

/// Handler for ZC_NOTIFY_MOVE_STOP packet
///
/// Processes server-initiated movement stop and emits an event
/// to immediately halt client-side movement interpolation.
pub struct MoveStopHandler;

impl PacketHandler<ZoneProtocol> for MoveStopHandler {
    type Packet = ZcNotifyMoveStopPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        context: &mut ZoneContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        info!(
            "Movement stopped by server for account {} at ({}, {})",
            packet.account_id, packet.x, packet.y
        );

        // TODO: Multi-Character Support
        // The packet.account_id identifies which character stopped, but we're
        // not passing it through to the event. Add account_id to the event
        // struct and use CharacterRegistry to look up the entity.
        //
        // Note: ZC_NOTIFY_MOVE_STOP doesn't include server_tick
        // Use current context tick for event
        let event = MovementStoppedByServer {
            x: packet.x,
            y: packet.y,
            server_tick: context.server_tick,
            // TODO: Pass account_id when multi-character support is implemented
        };

        event_writer.send_event(Box::new(event));

        Ok(())
    }
}
