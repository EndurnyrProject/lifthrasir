use crate::{
    domain::entities::types::ObjectType,
    infrastructure::networking::protocol::traits::ServerPacket,
    utils::decode_move_data,
};
use bytes::Buf;
use std::io;

pub const ZC_NOTIFY_MOVEENTRY: u16 = 0x09FD;

/// ZC_NOTIFY_MOVEENTRY (0x09FD) - Zone Server → Client
///
/// Sent when an entity (player, NPC, monster, etc.) appears in the client's view while moving.
/// This is one of three entity visibility packets:
/// - STANDENTRY (0x09FF): Entity is standing idle
/// - NEWENTRY (0x09FE): Entity is spawning/appearing
/// - MOVEENTRY (0x09FD): Entity is walking
///
/// This packet differs from STANDENTRY/NEWENTRY in that:
/// - It has a `move_start_time` field (u32) after the accessory field
/// - Uses movement data (6 bytes) instead of pos_dir for: src_x, src_y, dst_x, dst_y
/// - Does NOT have a `state` field
///
/// # Packet Structure
/// Variable-length packet with structure: [packet_id:u16][length:u16][data...]
///
/// All fields are little-endian.
#[derive(Debug, Clone)]
pub struct ZcNotifyMoveentryPacket {
    pub object_type: ObjectType,
    pub aid: u32,
    pub gid: u32,
    pub speed: u16,
    pub body_state: u16,
    pub health_state: u16,
    pub effect_state: u32,
    pub job: u16,
    pub head: u16,
    pub weapon: u32,
    pub shield: u32,
    pub accessory: u16,
    pub move_start_time: u32,
    pub accessory2: u16,
    pub accessory3: u16,
    pub head_palette: u16,
    pub body_palette: u16,
    pub head_dir: u16,
    pub robe: u16,
    pub guild_id: u32,
    pub guild_emblem_ver: u16,
    pub honor: u16,
    pub virtue: u32,
    pub is_pk_mode_on: u8,
    pub sex: u8,
    pub src_x: u16,
    pub src_y: u16,
    pub dst_x: u16,
    pub dst_y: u16,
    pub x_size: u8,
    pub y_size: u8,
    pub clevel: u16,
    pub font: u16,
    pub max_hp: u32,
    pub hp: u32,
    pub is_boss: u8,
    pub body: u16,
    pub name: String,
}

impl ServerPacket for ZcNotifyMoveentryPacket {
    const PACKET_ID: u16 = ZC_NOTIFY_MOVEENTRY;

    fn parse(data: &[u8]) -> io::Result<Self> {
        if data.len() < 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_NOTIFY_MOVEENTRY packet too short: expected at least 4 bytes, got {}",
                    data.len()
                ),
            ));
        }

        let mut buf = data;

        buf.advance(2);

        let packet_length = buf.get_u16_le();
        if data.len() < packet_length as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ZC_NOTIFY_MOVEENTRY incomplete: expected {} bytes, got {}",
                    packet_length,
                    data.len()
                ),
            ));
        }

        let object_type = ObjectType::from(buf.get_u8());
        let aid = buf.get_u32_le();
        let gid = buf.get_u32_le();
        let speed = buf.get_u16_le();
        let body_state = buf.get_u16_le();
        let health_state = buf.get_u16_le();
        let effect_state = buf.get_u32_le();
        let job = buf.get_u16_le();
        let head = buf.get_u16_le();
        let weapon = buf.get_u32_le();
        let shield = buf.get_u32_le();
        let accessory = buf.get_u16_le();
        let move_start_time = buf.get_u32_le();
        let accessory2 = buf.get_u16_le();
        let accessory3 = buf.get_u16_le();
        let head_palette = buf.get_u16_le();
        let body_palette = buf.get_u16_le();
        let head_dir = buf.get_u16_le();
        let robe = buf.get_u16_le();
        let guild_id = buf.get_u32_le();
        let guild_emblem_ver = buf.get_u16_le();
        let honor = buf.get_u16_le();
        let virtue = buf.get_u32_le();
        let is_pk_mode_on = buf.get_u8();
        let sex = buf.get_u8();

        let move_data = [
            buf.get_u8(),
            buf.get_u8(),
            buf.get_u8(),
            buf.get_u8(),
            buf.get_u8(),
            buf.get_u8(),
        ];
        let (src_x, src_y, dst_x, dst_y) = decode_move_data(move_data);

        let x_size = buf.get_u8();
        let y_size = buf.get_u8();
        let clevel = buf.get_u16_le();
        let font = buf.get_u16_le();
        let max_hp = buf.get_u32_le();
        let hp = buf.get_u32_le();
        let is_boss = buf.get_u8();
        let body = buf.get_u16_le();

        let mut name_bytes = [0u8; 24];
        buf.copy_to_slice(&mut name_bytes);
        let name_end = name_bytes.iter().position(|&b| b == 0).unwrap_or(24);
        let name = String::from_utf8_lossy(&name_bytes[..name_end]).to_string();

        Ok(Self {
            object_type,
            aid,
            gid,
            speed,
            body_state,
            health_state,
            effect_state,
            job,
            head,
            weapon,
            shield,
            accessory,
            move_start_time,
            accessory2,
            accessory3,
            head_palette,
            body_palette,
            head_dir,
            robe,
            guild_id,
            guild_emblem_ver,
            honor,
            virtue,
            is_pk_mode_on,
            sex,
            src_x,
            src_y,
            dst_x,
            dst_y,
            x_size,
            y_size,
            clevel,
            font,
            max_hp,
            hp,
            is_boss,
            body,
            name,
        })
    }

    fn packet_id(&self) -> u16 {
        Self::PACKET_ID
    }
}
