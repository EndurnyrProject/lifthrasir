use crate::infrastructure::networking::{
    errors::NetworkError,
    protocol::{
        traits::{EventWriter, PacketHandler},
        zone::{
            protocol::{ZoneContext, ZoneProtocol},
            server_packets::{ZcNotifyTime2Packet, ZcNotifyTimePacket},
        },
    },
};
use bevy::prelude::*;

/// Trait for packets that contain server time synchronization data
pub trait TimeSyncPacket {
    fn server_time(&self) -> u32;
}

impl TimeSyncPacket for ZcNotifyTimePacket {
    fn server_time(&self) -> u32 {
        self.server_tick
    }
}

impl TimeSyncPacket for ZcNotifyTime2Packet {
    fn server_time(&self) -> u32 {
        self.server_time
    }
}

/// Updates time synchronization in ZoneContext
///
/// Shared logic for handling time sync responses from the server.
/// Calculates and updates the time offset for accurate client-server synchronization.
fn update_time_sync(server_time: u32, context: &mut ZoneContext) {
    let client_time = crate::utils::time::current_milliseconds();

    debug!(
        "Time sync response: server_time={}, client_time={}, offset={}",
        server_time,
        client_time,
        server_time.wrapping_sub(client_time) as i32
    );

    // Note: We don't have the original request time stored, so we use current time
    // This means the offset calculation doesn't account for round-trip time.
    // For more accurate sync, we'd need to store request times in context.
    context.update_time_offset(server_time, client_time);

    debug!("Time offset updated to: {}", context.time_offset);
}

/// Handler for ZC_NOTIFY_TIME2 (0x02C2) packet
///
/// Processes server time synchronization responses and updates the time offset
/// in ZoneContext. This enables accurate client-server time synchronization for
/// movement interpolation and other time-sensitive features.
pub struct TimeSyncHandler;

impl PacketHandler<ZoneProtocol> for TimeSyncHandler {
    type Packet = ZcNotifyTime2Packet;

    fn handle(
        &self,
        packet: Self::Packet,
        context: &mut ZoneContext,
        _event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        update_time_sync(packet.server_time(), context);
        Ok(())
    }
}

/// Handler for ZC_NOTIFY_TIME (0x007F) packet - Legacy version
///
/// Processes server time synchronization responses from legacy servers.
/// Has the same structure and behavior as ZC_NOTIFY_TIME2 but with a different packet ID.
pub struct TimeSyncLegacyHandler;

impl PacketHandler<ZoneProtocol> for TimeSyncLegacyHandler {
    type Packet = ZcNotifyTimePacket;

    fn handle(
        &self,
        packet: Self::Packet,
        context: &mut ZoneContext,
        _event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        update_time_sync(packet.server_time(), context);
        Ok(())
    }
}
