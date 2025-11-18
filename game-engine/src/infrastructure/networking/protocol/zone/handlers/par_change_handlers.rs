use crate::infrastructure::networking::{
    errors::NetworkError,
    protocol::{
        traits::{EventWriter, PacketHandler},
        zone::{
            protocol::{ZoneContext, ZoneProtocol},
            server_packets::{ZcLongparChangePacket, ZcParChangePacket},
        },
    },
};
use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::auto_add_message;

/// Event emitted when a character parameter changes
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::domain::entities::character::UnifiedCharacterEntityPlugin)]
pub struct ParameterChanged {
    pub var_id: u16,
    pub value: u32,
}

/// Handler for ZC_PAR_CHANGE packet
///
/// Processes character parameter changes from the zone server.
/// Currently logs the changes for debugging purposes.
pub struct ParChangeHandler;

impl PacketHandler<ZoneProtocol> for ParChangeHandler {
    type Packet = ZcParChangePacket;

    fn handle(
        &self,
        packet: Self::Packet,
        _context: &mut ZoneContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        debug!(
            "ZC_PAR_CHANGE received: var_id=0x{:04X}, value={}",
            packet.var_id, packet.value
        );

        let event = ParameterChanged {
            var_id: packet.var_id,
            value: packet.value,
        };

        event_writer.send_event(Box::new(event));
        debug!("ParameterChanged event sent");

        Ok(())
    }
}

/// Handler for ZC_LONGPAR_CHANGE packet
///
/// Processes character parameter changes (long values) from the zone server.
/// Currently logs the changes for debugging purposes.
pub struct LongparChangeHandler;

impl PacketHandler<ZoneProtocol> for LongparChangeHandler {
    type Packet = ZcLongparChangePacket;

    fn handle(
        &self,
        packet: Self::Packet,
        _context: &mut ZoneContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        debug!(
            "ZC_LONGPAR_CHANGE received: var_id=0x{:04X}, value={}",
            packet.var_id, packet.value
        );

        let event = ParameterChanged {
            var_id: packet.var_id,
            value: packet.value,
        };

        event_writer.send_event(Box::new(event));
        debug!("ParameterChanged event sent (long)");

        Ok(())
    }
}
