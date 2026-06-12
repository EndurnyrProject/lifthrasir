use crate::{
    domain::combat::events::{CombatActionReceived, CombatActionType, EntityHpReceived},
    infrastructure::networking::{
        errors::NetworkError,
        protocol::{
            traits::{EventWriter, PacketHandler},
            zone::{
                protocol::{ZoneContext, ZoneProtocol},
                server_packets::{ZcHpInfoPacket, ZcNotifyActPacket},
            },
        },
    },
};
use bevy::prelude::*;

/// Handler for ZC_NOTIFY_ACT packet
///
/// Processes combat action notifications from the server and emits
/// CombatActionReceived events for the combat system to handle.
pub struct CombatActionHandler;

impl PacketHandler<ZoneProtocol> for CombatActionHandler {
    type Packet = ZcNotifyActPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        context: &mut ZoneContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        context.server_tick = packet.server_tick;

        let action_type = CombatActionType::from(packet.action_type);
        let is_sp_damage = packet.is_sp_damage != 0;

        info!(
            "[HANDLER] Combat action: src={} -> target={}, action={:?}, damage={}, damage2={}, div={}",
            packet.src_id, packet.target_id, action_type, packet.damage, packet.damage2, packet.div
        );

        let event = CombatActionReceived {
            src_id: packet.src_id,
            target_id: packet.target_id,
            server_tick: packet.server_tick,
            src_speed: packet.src_speed,
            dmg_speed: packet.dmg_speed,
            damage: packet.damage,
            is_sp_damage,
            div: packet.div,
            action_type,
            damage2: packet.damage2,
        };

        event_writer.send_event(Box::new(event));

        Ok(())
    }
}

/// Handler for ZC_HP_INFO packet (0x0977)
///
/// Processes HP information updates for any entity type (players, monsters, NPCs).
/// Emits EntityHpReceived events for UI/game systems to update HP displays.
pub struct HpInfoHandler;

impl PacketHandler<ZoneProtocol> for HpInfoHandler {
    type Packet = ZcHpInfoPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        _context: &mut ZoneContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        let event = EntityHpReceived {
            entity_id: packet.id,
            hp: packet.hp,
            max_hp: packet.max_hp,
        };

        event_writer.send_event(Box::new(event));

        Ok(())
    }
}
