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
    pub aid: u32,
    pub src_x: u16,
    pub src_y: u16,
    pub dest_x: u16,
    pub dest_y: u16,
    pub server_tick: u32,
}

/// Event emitted when server forces movement to stop
#[derive(Message, Debug, Clone)]
pub struct MovementStoppedByServer {
    pub aid: u32,
    pub x: u16,
    pub y: u16,
    pub server_tick: u32,
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

        // ZC_NOTIFY_PLAYERMOVE is for local player only, use account_id from context
        let Some(account_id) = context.account_id else {
            error!("Received movement packet but account_id not set in context");
            return Err(NetworkError::HandlerFailure {
                id: 0x0087,
                reason: "Account ID not available in zone context".to_string(),
            });
        };

        let event = MovementConfirmedByServer {
            aid: account_id,
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

        let event = MovementStoppedByServer {
            aid: packet.account_id,
            x: packet.x,
            y: packet.y,
            server_tick: context.server_tick,
        };

        event_writer.send_event(Box::new(event));

        Ok(())
    }
}
