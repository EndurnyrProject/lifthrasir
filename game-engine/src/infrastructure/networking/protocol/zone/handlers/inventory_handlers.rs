use crate::infrastructure::networking::{
    errors::NetworkError,
    protocol::{
        traits::{EventWriter, PacketHandler},
        zone::{
            protocol::ZoneContext,
            protocol::ZoneProtocol,
            server_packets::{ZcEquipitemListPacket, ZcNormalItemlistPacket},
        },
    },
};
use bevy::prelude::*;

/// Handler for ZC_NORMAL_ITEMLIST packet
///
/// Processes the player's normal (non-equipped) inventory items received from the server.
/// This packet is typically sent after entering the zone/map.
///
/// Currently logs the received items for debugging purposes.
/// Future implementation will populate inventory UI and game state.
pub struct NormalItemlistHandler;

impl PacketHandler<ZoneProtocol> for NormalItemlistHandler {
    type Packet = ZcNormalItemlistPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        _context: &mut ZoneContext,
        _event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        debug!(
            "Received normal item list with {} items",
            packet.items.len()
        );

        for (i, item) in packet.items.iter().enumerate() {
            debug!(
                "  Item {}: nameid={}, amount={}, type={}, identify={}, refine={}, cards=[{},{},{},{}]",
                i,
                item.nameid,
                item.amount,
                item.item_type,
                item.identify,
                item.refine,
                item.card0,
                item.card1,
                item.card2,
                item.card3
            );

            if item.expire_time > 0 {
                debug!("    Expires at: {}", item.expire_time);
            }

            if item.favorite != 0 {
                debug!("    Marked as favorite");
            }

            if item.bound != 0 {
                debug!("    Bound type: {}", item.bound);
            }
        }

        Ok(())
    }
}

/// Handler for ZC_EQUIPITEM_LIST packet
///
/// Processes the player's equipped items received from the server.
/// This packet is typically sent after the normal itemlist during login sequence.
///
/// Currently logs the received equipped items for debugging purposes.
/// Future implementation will populate equipment UI and game state.
pub struct EquipitemListHandler;

impl PacketHandler<ZoneProtocol> for EquipitemListHandler {
    type Packet = ZcEquipitemListPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        _context: &mut ZoneContext,
        _event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        debug!(
            "Received equipped item list with {} items",
            packet.items.len()
        );

        for (i, item) in packet.items.iter().enumerate() {
            debug!(
                "  Equipped Item {}: nameid={}, type={}, identify={}, refine={}, cards=[{},{},{},{}]",
                i,
                item.nameid,
                item.item_type,
                item.identify,
                item.refine,
                item.card0,
                item.card1,
                item.card2,
                item.card3
            );

            debug!("    Equipment slot: 0x{:04X}", item.location);

            if item.location2 != 0 {
                debug!("    Switch slot: 0x{:04X}", item.location2);
            }

            if item.wlv > 0 {
                debug!("    Weapon level: {}", item.wlv);
            }

            if item.expire_time > 0 {
                debug!("    Expires at: {}", item.expire_time);
            }

            if item.favorite != 0 {
                debug!("    Marked as favorite");
            }

            if item.bound != 0 {
                debug!("    Bound type: {}", item.bound);
            }
        }

        Ok(())
    }
}
