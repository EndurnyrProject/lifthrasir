use crate::domain::inventory::{
    InventoryDumpCompleted, InventoryDumpStarted, InventoryItemsReceived, Item,
};
use crate::infrastructure::networking::{
    errors::NetworkError,
    protocol::{
        traits::{EventWriter, PacketHandler},
        zone::{
            protocol::ZoneContext,
            protocol::ZoneProtocol,
            server_packets::{
                ZcInventoryEndPacket, ZcInventoryItemlistEquipPacket,
                ZcInventoryItemlistNormalPacket, ZcInventoryStartPacket,
            },
        },
    },
};
use bevy::prelude::*;

/// Handler for ZC_INVENTORY_START (0x0B08)
///
/// Begin-transaction marker for an inventory dump. Emits `InventoryDumpStarted`
/// so the domain system can clear the resource before items arrive.
pub struct InventoryStartHandler;

impl PacketHandler<ZoneProtocol> for InventoryStartHandler {
    type Packet = ZcInventoryStartPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        _context: &mut ZoneContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        debug!("Inventory dump started for '{}'", packet.name);
        event_writer.send_event(Box::new(InventoryDumpStarted));
        Ok(())
    }
}

/// Handler for ZC_INVENTORY_ITEMLIST_NORMAL (0x0B09)
///
/// Stackable (non-equipped) items. Maps each wire item into the domain `Item`
/// and emits `InventoryItemsReceived`.
pub struct InventoryItemlistNormalHandler;

impl PacketHandler<ZoneProtocol> for InventoryItemlistNormalHandler {
    type Packet = ZcInventoryItemlistNormalPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        _context: &mut ZoneContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        debug!("Received {} normal inventory items", packet.items.len());
        let items = packet.items.iter().map(Item::from).collect();
        event_writer.send_event(Box::new(InventoryItemsReceived { items }));
        Ok(())
    }
}

/// Handler for ZC_INVENTORY_ITEMLIST_EQUIP (0x0B0A)
///
/// Equippable items. Maps each wire item into the domain `Item` and emits
/// `InventoryItemsReceived`.
pub struct InventoryItemlistEquipHandler;

impl PacketHandler<ZoneProtocol> for InventoryItemlistEquipHandler {
    type Packet = ZcInventoryItemlistEquipPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        _context: &mut ZoneContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        debug!("Received {} equip inventory items", packet.items.len());
        let items = packet.items.iter().map(Item::from).collect();
        event_writer.send_event(Box::new(InventoryItemsReceived { items }));
        Ok(())
    }
}

/// Handler for ZC_INVENTORY_END (0x0B0B)
///
/// End-of-transaction marker. Emits `InventoryDumpCompleted` so the domain
/// system can mark the resource ready.
pub struct InventoryEndHandler;

impl PacketHandler<ZoneProtocol> for InventoryEndHandler {
    type Packet = ZcInventoryEndPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        _context: &mut ZoneContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        debug!("Inventory dump completed (flag {})", packet.flag);
        event_writer.send_event(Box::new(InventoryDumpCompleted));
        Ok(())
    }
}
