use crate::{
    domain::entities::spawning::events::{RequestEntityVanish, SpawnEntity},
    infrastructure::networking::{
        errors::NetworkError,
        protocol::{
            traits::{EventWriter, PacketHandler},
            zone::{
                protocol::{ZoneContext, ZoneProtocol},
                server_packets::{
                    ZcNotifyMoveentryPacket, ZcNotifyNewentryPacket, ZcNotifyStandentryPacket,
                    ZcNotifyVanishPacket,
                },
            },
        },
    },
};
use bevy::prelude::*;

/// Calculate movement direction (0-7) from a delta vector
fn calculate_direction_from_vector(dx: i32, dy: i32) -> u8 {
    use std::f32::consts::PI;

    if dx == 0 && dy == 0 {
        return 0;
    }

    let angle = (dy as f32).atan2(dx as f32);
    let normalized = (angle + 2.0 * PI) % (2.0 * PI);
    let direction_index = ((normalized / (PI / 4.0)).round() as i32) % 8;
    direction_index as u8
}

/// Handler for ZC_NOTIFY_STANDENTRY packet
///
/// Processes entities appearing in view while standing idle.
pub struct StandentryHandler;

impl PacketHandler<ZoneProtocol> for StandentryHandler {
    type Packet = ZcNotifyStandentryPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        context: &mut ZoneContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        debug!(
            "Entity standing: {} ({:?}) at ({}, {})",
            packet.name, packet.object_type, packet.x, packet.y
        );

        let event = SpawnEntity {
            aid: packet.aid,
            gid: packet.gid,
            object_type: packet.object_type,
            name: packet.name,

            position: (packet.x, packet.y),
            direction: packet.dir,
            destination: None,
            move_start_time: None,
            current_server_tick: context.get_server_time(),

            job: packet.job,
            head: packet.head,
            body: packet.body,
            gender: packet.sex,
            head_palette: packet.head_palette,
            body_palette: packet.body_palette,

            weapon: packet.weapon,
            shield: packet.shield,
            head_bottom: packet.accessory,
            head_mid: packet.accessory2,
            head_top: packet.accessory3,
            robe: packet.robe,

            hp: packet.hp,
            max_hp: packet.max_hp,
            speed: packet.speed,
            level: packet.clevel,
        };

        event_writer.send_event(Box::new(event));
        Ok(())
    }
}

/// Handler for ZC_NOTIFY_NEWENTRY packet
///
/// Processes entities spawning/appearing in view.
pub struct NewentryHandler;

impl PacketHandler<ZoneProtocol> for NewentryHandler {
    type Packet = ZcNotifyNewentryPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        context: &mut ZoneContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        debug!(
            "Entity new entry: {} ({:?}) at ({}, {})",
            packet.name, packet.object_type, packet.x, packet.y
        );

        let event = SpawnEntity {
            aid: packet.aid,
            gid: packet.gid,
            object_type: packet.object_type,
            name: packet.name,

            position: (packet.x, packet.y),
            direction: packet.dir,
            destination: None,
            move_start_time: None,
            current_server_tick: context.get_server_time(),

            job: packet.job,
            head: packet.head,
            body: packet.body,
            gender: packet.sex,
            head_palette: packet.head_palette,
            body_palette: packet.body_palette,

            weapon: packet.weapon,
            shield: packet.shield,
            head_bottom: packet.accessory,
            head_mid: packet.accessory2,
            head_top: packet.accessory3,
            robe: packet.robe,

            hp: packet.hp,
            max_hp: packet.max_hp,
            speed: packet.speed,
            level: packet.clevel,
        };

        event_writer.send_event(Box::new(event));
        Ok(())
    }
}

/// Handler for ZC_NOTIFY_MOVEENTRY packet
///
/// Processes entities appearing in view while moving.
pub struct MoveentryHandler;

impl PacketHandler<ZoneProtocol> for MoveentryHandler {
    type Packet = ZcNotifyMoveentryPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        context: &mut ZoneContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        debug!(
            "Entity move entry: {} ({:?}) moving ({}, {}) -> ({}, {}) at server tick {}",
            packet.name,
            packet.object_type,
            packet.src_x,
            packet.src_y,
            packet.dst_x,
            packet.dst_y,
            packet.move_start_time
        );

        let dx = packet.dst_x as i32 - packet.src_x as i32;
        let dy = packet.dst_y as i32 - packet.src_y as i32;
        let direction = calculate_direction_from_vector(dx, dy);

        let event = SpawnEntity {
            aid: packet.aid,
            gid: packet.gid,
            object_type: packet.object_type,
            name: packet.name,

            position: (packet.src_x, packet.src_y),
            direction,
            destination: Some((packet.dst_x, packet.dst_y)),
            move_start_time: Some(packet.move_start_time),
            current_server_tick: context.get_server_time(),

            job: packet.job,
            head: packet.head,
            body: packet.body,
            gender: packet.sex,
            head_palette: packet.head_palette,
            body_palette: packet.body_palette,

            weapon: packet.weapon,
            shield: packet.shield,
            head_bottom: packet.accessory,
            head_mid: packet.accessory2,
            head_top: packet.accessory3,
            robe: packet.robe,

            hp: packet.hp,
            max_hp: packet.max_hp,
            speed: packet.speed,
            level: packet.clevel,
        };

        event_writer.send_event(Box::new(event));
        Ok(())
    }
}

/// Handler for ZC_NOTIFY_VANISH packet
///
/// Processes entities disappearing from view (moved out of range, died, logged out, or teleported).
/// Emits RequestEntityVanish event which will be handled by a system that can check movement state.
pub struct VanishHandler;

impl PacketHandler<ZoneProtocol> for VanishHandler {
    type Packet = ZcNotifyVanishPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        _context: &mut ZoneContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        let vanish_reason = match packet.vanish_type {
            0 => "out of sight",
            1 => "died",
            2 => "logged out",
            3 => "teleported",
            _ => "unknown reason",
        };

        debug!(
            "Entity vanish requested: GID {} ({})",
            packet.gid, vanish_reason
        );

        // Emit RequestEntityVanish instead of DespawnEntity
        // A system will check if entity is moving and defer despawn if needed
        // Note: packet.gid actually contains AID (in Ragnarok Online, GID == AID)
        event_writer.send_event(Box::new(RequestEntityVanish {
            aid: packet.gid,
            vanish_type: packet.vanish_type,
        }));

        Ok(())
    }
}
