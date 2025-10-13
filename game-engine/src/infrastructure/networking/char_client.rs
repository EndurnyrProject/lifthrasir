use super::errors::NetworkError;
use super::protocols::ro_char::{
    ChCharlistReqPacket, ChDeleteCharPacket, ChEnterPacket, ChMakeCharPacket, ChPingPacket,
    ChSelectCharPacket, CharServerResponse, CharacterInfo, HcAcceptEnterPacket,
    HcAckCharinfoPerPagePacket, HcBlockCharacterPacket, HcCharacterListPacket,
    HcNotifyZonesvrPacket, HcSecondPasswdLoginPacket, HC_ACCEPT_DELETECHAR, HC_ACCEPT_ENTER,
    HC_ACCEPT_MAKECHAR, HC_ACK_CHARINFO_PER_PAGE, HC_BLOCK_CHARACTER, HC_CHARACTER_LIST,
    HC_NOTIFY_ZONESVR, HC_PING, HC_REFUSE_DELETECHAR, HC_REFUSE_MAKECHAR, HC_SECOND_PASSWD_LOGIN,
};
use super::session::UserSession;
use bevy::prelude::*;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(12);
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);

fn get_packet_size(packet_id: u16) -> Option<usize> {
    match packet_id {
        // Fixed size packets
        HC_CHARACTER_LIST => Some(29),
        HC_NOTIFY_ZONESVR => Some(28),
        HC_PING => Some(6),
        HC_ACCEPT_DELETECHAR => Some(2),
        HC_REFUSE_DELETECHAR => Some(3),
        HC_REFUSE_MAKECHAR => Some(3),
        HC_SECOND_PASSWD_LOGIN => Some(12),
        // Variable length packets return None (need to read length field)
        HC_ACCEPT_ENTER | HC_BLOCK_CHARACTER | HC_ACCEPT_MAKECHAR => None,
        // Unknown packets
        _ => None,
    }
}

#[derive(Resource)]
pub struct CharServerClient {
    connection: Option<TcpStream>,
    session: UserSession,
    last_keepalive: Instant,
    pub characters: Vec<CharacterInfo>,
    received_account_ack: bool,
}

impl CharServerClient {
    pub fn new(session: UserSession) -> Self {
        Self {
            connection: None,
            session,
            last_keepalive: Instant::now(),
            characters: Vec::new(),
            received_account_ack: false,
        }
    }

    pub fn connect(&mut self, server_ip: &str, server_port: u16) -> Result<(), NetworkError> {
        info!(
            "Connecting to character server at {}:{}",
            server_ip, server_port
        );

        let addr = format!("{}:{}", server_ip, server_port);
        let socket_addr = addr
            .parse::<std::net::SocketAddr>()
            .map_err(|_| NetworkError::InvalidPacket)?;
        let stream = TcpStream::connect_timeout(&socket_addr, CONNECTION_TIMEOUT)?;
        stream.set_nonblocking(true)?;

        self.connection = Some(stream);
        self.last_keepalive = Instant::now();
        self.received_account_ack = false; // Reset the flag

        // Send CH_ENTER packet immediately after connection
        self.send_enter_packet()?;

        Ok(())
    }

    pub fn disconnect(&mut self) {
        if let Some(stream) = self.connection.take() {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
        self.characters.clear();
    }

    pub fn is_connected(&self) -> bool {
        self.connection.is_some()
    }

    fn send_enter_packet(&mut self) -> Result<(), NetworkError> {
        let packet = ChEnterPacket {
            account_id: self.session.tokens.account_id,
            login_id1: self.session.tokens.login_id1,
            login_id2: self.session.tokens.login_id2,
            unknown: 0,
            sex: self.session.sex,
        };

        self.send_packet(&packet.serialize())?;
        info!("Sent CH_ENTER packet to character server");
        Ok(())
    }

    pub fn request_character_list(&mut self) -> Result<Vec<CharacterInfo>, NetworkError> {
        // The character list is received automatically after CH_ENTER
        // This function just returns the cached list
        Ok(self.characters.clone())
    }

    pub fn request_charlist(&mut self) -> Result<(), NetworkError> {
        self.send_packet(&ChCharlistReqPacket::serialize())?;
        info!("Sent CH_CHARLIST_REQ to refresh character list");
        Ok(())
    }

    pub fn select_character(&mut self, char_num: u8) -> Result<(), NetworkError> {
        let packet = ChSelectCharPacket { char_num };
        self.send_packet(&packet.serialize())?;
        info!("Selected character slot {}", char_num);
        Ok(())
    }

    pub fn create_character(
        &mut self,
        creation_data: ChMakeCharPacket,
    ) -> Result<(), NetworkError> {
        self.send_packet(&creation_data.serialize())?;
        info!(
            "Sent character creation request for '{}'",
            creation_data.name
        );
        Ok(())
    }

    pub fn delete_character(&mut self, char_id: u32, email: String) -> Result<(), NetworkError> {
        let packet = ChDeleteCharPacket { char_id, email };
        self.send_packet(&packet.serialize())?;
        info!("Sent character deletion request for ID {}", char_id);
        Ok(())
    }

    pub fn send_keepalive(&mut self) -> Result<(), NetworkError> {
        if self.last_keepalive.elapsed() >= KEEPALIVE_INTERVAL {
            self.send_packet(&ChPingPacket::serialize())?;
            self.last_keepalive = Instant::now();
            debug!("Sent keepalive to character server");
        }
        Ok(())
    }

    fn send_packet(&mut self, data: &[u8]) -> Result<(), NetworkError> {
        if let Some(ref mut stream) = self.connection {
            stream.write_all(data)?;
            stream.flush()?;
            Ok(())
        } else {
            Err(NetworkError::UnexpectedDisconnect)
        }
    }

    pub fn receive_packets(&mut self) -> Result<Vec<CharServerResponse>, NetworkError> {
        let mut responses = Vec::new();

        if let Some(ref mut stream) = self.connection {
            let mut buffer = [0u8; 4096];

            loop {
                match stream.read(&mut buffer) {
                    Ok(0) => {
                        // Connection closed
                        self.disconnect();
                        return Err(NetworkError::UnexpectedDisconnect);
                    }
                    Ok(n) => {
                        debug!("Received {} bytes from character server", n);
                        let data = &buffer[..n];
                        let mut cursor = 0;

                        // Handle the special 4-byte account ID acknowledgment first
                        if !self.received_account_ack && data.len() >= 4 {
                            // First 4 bytes after CH_ENTER is account ID acknowledgment
                            let account_id =
                                u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                            debug!("Received account ID acknowledgment: {}", account_id);

                            if account_id == self.session.tokens.account_id {
                                info!("Account ID verified by character server");
                                self.received_account_ack = true;
                                cursor = 4; // Skip the 4-byte ack

                                // If there's no more data, return
                                if cursor >= data.len() {
                                    break;
                                }
                            } else {
                                error!(
                                    "Account ID mismatch! Expected: {}, Got: {}",
                                    self.session.tokens.account_id, account_id
                                );
                                return Err(NetworkError::InvalidPacket);
                            }
                        }

                        while cursor < data.len() {
                            if data.len() - cursor < 2 {
                                break; // Not enough data for packet ID
                            }

                            let packet_id = u16::from_le_bytes([data[cursor], data[cursor + 1]]);
                            debug!(
                                "Processing packet ID: 0x{:04X} at offset {}",
                                packet_id, cursor
                            );

                            match packet_id {
                                HC_CHARACTER_LIST => {
                                    const PACKET_SIZE: usize = 29;
                                    if data.len() - cursor < PACKET_SIZE {
                                        debug!("Incomplete HC_CHARACTER_LIST packet");
                                        break;
                                    }

                                    match HcCharacterListPacket::parse(
                                        &data[cursor..cursor + PACKET_SIZE],
                                    ) {
                                        Ok(packet) => {
                                            info!(
                                                "Received character slot configuration - normal: {}, premium: {}, valid: {}",
                                                packet.normal_slots,
                                                packet.premium_slots,
                                                packet.valid_slots
                                            );
                                            responses
                                                .push(CharServerResponse::HcCharacterList(packet));
                                            cursor += PACKET_SIZE;
                                        }
                                        Err(e) => {
                                            error!(
                                                "Failed to parse HC_CHARACTER_LIST packet: {:?}",
                                                e
                                            );
                                            cursor += PACKET_SIZE;
                                        }
                                    }
                                }
                                HC_ACCEPT_ENTER => {
                                    // Check if we have enough bytes for the packet length field
                                    if data.len() - cursor < 4 {
                                        debug!("Not enough data for HC_ACCEPT_ENTER packet length");
                                        break;
                                    }

                                    // Read packet length from bytes 2-3
                                    let packet_len =
                                        u16::from_le_bytes([data[cursor + 2], data[cursor + 3]])
                                            as usize;
                                    debug!("HC_ACCEPT_ENTER packet length: {}", packet_len);

                                    // Check if we have the full packet
                                    if data.len() - cursor < packet_len {
                                        debug!(
                                            "Incomplete HC_ACCEPT_ENTER packet: have {}, need {}",
                                            data.len() - cursor,
                                            packet_len
                                        );
                                        break;
                                    }

                                    // Parse the complete packet
                                    match HcAcceptEnterPacket::parse(
                                        &data[cursor..cursor + packet_len],
                                    ) {
                                        Ok(packet) => {
                                            info!(
                                                "Successfully parsed HC_ACCEPT_ENTER with {} characters",
                                                packet.characters.len()
                                            );
                                            self.characters = packet.characters.clone();
                                            responses
                                                .push(CharServerResponse::HcAcceptEnter(packet));
                                            cursor += packet_len;
                                        }
                                        Err(e) => {
                                            error!(
                                                "Failed to parse HC_ACCEPT_ENTER packet: {:?}",
                                                e
                                            );
                                            cursor += packet_len; // Skip the malformed packet
                                        }
                                    }
                                }
                                HC_NOTIFY_ZONESVR => {
                                    const PACKET_SIZE: usize = 28;
                                    if data.len() - cursor < PACKET_SIZE {
                                        debug!("Incomplete HC_NOTIFY_ZONESVR packet");
                                        break;
                                    }

                                    match HcNotifyZonesvrPacket::parse(
                                        &data[cursor..cursor + PACKET_SIZE],
                                    ) {
                                        Ok(packet) => {
                                            info!(
                                                "Received zone server info for map: {}",
                                                packet.map_name
                                            );
                                            responses
                                                .push(CharServerResponse::HcNotifyZonesvr(packet));
                                            cursor += PACKET_SIZE;
                                        }
                                        Err(e) => {
                                            error!(
                                                "Failed to parse HC_NOTIFY_ZONESVR packet: {:?}",
                                                e
                                            );
                                            cursor += PACKET_SIZE;
                                        }
                                    }
                                }
                                HC_PING => {
                                    const PACKET_SIZE: usize = 6;
                                    if data.len() - cursor < PACKET_SIZE {
                                        debug!("Incomplete HC_PING packet");
                                        break;
                                    }
                                    debug!("Received HC_PING");
                                    responses.push(CharServerResponse::HcPing);
                                    cursor += PACKET_SIZE;
                                }
                                HC_ACCEPT_MAKECHAR => {
                                    // Character creation success - parse character info
                                    const MIN_SIZE: usize = 175; // Minimum character info size
                                    if data.len() - cursor < MIN_SIZE {
                                        debug!("Incomplete HC_ACCEPT_MAKECHAR packet");
                                        break;
                                    }

                                    match CharacterInfo::parse(&data[cursor + 2..]) {
                                        Ok(char_info) => {
                                            info!(
                                                "Character '{}' created successfully",
                                                char_info.name
                                            );
                                            responses.push(CharServerResponse::HcAcceptMakeChar(
                                                char_info,
                                            ));
                                            cursor += MIN_SIZE + 2;
                                        }
                                        Err(e) => {
                                            error!("Failed to parse character info: {:?}", e);
                                            cursor += MIN_SIZE + 2;
                                        }
                                    }
                                }
                                HC_REFUSE_MAKECHAR => {
                                    const PACKET_SIZE: usize = 3;
                                    if data.len() - cursor < PACKET_SIZE {
                                        debug!("Incomplete HC_REFUSE_MAKECHAR packet");
                                        break;
                                    }
                                    let error_code = data[cursor + 2];
                                    warn!("Character creation failed with error: {}", error_code);
                                    responses
                                        .push(CharServerResponse::HcRefuseMakeChar(error_code));
                                    cursor += PACKET_SIZE;
                                }
                                HC_ACCEPT_DELETECHAR => {
                                    const PACKET_SIZE: usize = 2;
                                    info!("Character deleted successfully");
                                    responses.push(CharServerResponse::HcAcceptDeleteChar);
                                    cursor += PACKET_SIZE;
                                }
                                HC_REFUSE_DELETECHAR => {
                                    const PACKET_SIZE: usize = 3;
                                    if data.len() - cursor < PACKET_SIZE {
                                        debug!("Incomplete HC_REFUSE_DELETECHAR packet");
                                        break;
                                    }
                                    let error_code = data[cursor + 2];
                                    warn!("Character deletion failed with error: {}", error_code);
                                    responses
                                        .push(CharServerResponse::HcRefuseDeleteChar(error_code));
                                    cursor += PACKET_SIZE;
                                }
                                HC_BLOCK_CHARACTER => {
                                    // Variable length packet
                                    if data.len() - cursor < 4 {
                                        debug!("Incomplete HC_BLOCK_CHARACTER packet header");
                                        break;
                                    }

                                    let packet_len =
                                        u16::from_le_bytes([data[cursor + 2], data[cursor + 3]])
                                            as usize;
                                    debug!("HC_BLOCK_CHARACTER packet length: {}", packet_len);

                                    if data.len() - cursor < packet_len {
                                        debug!("Incomplete HC_BLOCK_CHARACTER packet");
                                        break;
                                    }

                                    match HcBlockCharacterPacket::parse(
                                        &data[cursor..cursor + packet_len],
                                    ) {
                                        Ok(packet) => {
                                            info!(
                                                "Received blocked character list with {} entries",
                                                packet.blocked_chars.len()
                                            );
                                            responses
                                                .push(CharServerResponse::HcBlockCharacter(packet));
                                            cursor += packet_len;
                                        }
                                        Err(e) => {
                                            error!(
                                                "Failed to parse HC_BLOCK_CHARACTER packet: {:?}",
                                                e
                                            );
                                            cursor += packet_len;
                                        }
                                    }
                                }
                                HC_SECOND_PASSWD_LOGIN => {
                                    const PACKET_SIZE: usize = 12;
                                    if data.len() - cursor < PACKET_SIZE {
                                        debug!("Incomplete HC_SECOND_PASSWD_LOGIN packet");
                                        break;
                                    }

                                    match HcSecondPasswdLoginPacket::parse(
                                        &data[cursor..cursor + PACKET_SIZE],
                                    ) {
                                        Ok(packet) => {
                                            info!(
                                                "Received pincode state: {} - {}",
                                                packet.state,
                                                packet.state_description()
                                            );
                                            responses.push(
                                                CharServerResponse::HcSecondPasswdLogin(packet),
                                            );
                                            cursor += PACKET_SIZE;
                                        }
                                        Err(e) => {
                                            error!(
                                                "Failed to parse HC_SECOND_PASSWD_LOGIN packet: {:?}",
                                                e
                                            );
                                            cursor += PACKET_SIZE;
                                        }
                                    }
                                }
                                HC_ACK_CHARINFO_PER_PAGE => {
                                    // Variable length packet - check for packet length
                                    if data.len() - cursor < 4 {
                                        debug!("Incomplete HC_ACK_CHARINFO_PER_PAGE packet header");
                                        break;
                                    }

                                    let packet_len =
                                        u16::from_le_bytes([data[cursor + 2], data[cursor + 3]])
                                            as usize;
                                    debug!(
                                        "HC_ACK_CHARINFO_PER_PAGE packet length: {}",
                                        packet_len
                                    );

                                    if data.len() - cursor < packet_len {
                                        debug!(
                                            "Incomplete HC_ACK_CHARINFO_PER_PAGE packet: have {}, need {}",
                                            data.len() - cursor,
                                            packet_len
                                        );
                                        break;
                                    }

                                    match HcAckCharinfoPerPagePacket::parse(
                                        &data[cursor..cursor + packet_len],
                                    ) {
                                        Ok(packet) => {
                                            // Only update character list if packet contains characters
                                            // Empty packets are end-of-pagination markers
                                            if !packet.characters.is_empty() {
                                                info!(
                                                    "Received character list refresh with {} characters",
                                                    packet.characters.len()
                                                );
                                                self.characters = packet.characters.clone();
                                                responses.push(
                                                    CharServerResponse::HcAckCharinfoPerPage(
                                                        packet,
                                                    ),
                                                );
                                            }

                                            cursor += packet_len;
                                        }
                                        Err(e) => {
                                            error!(
                                                "Failed to parse HC_ACK_CHARINFO_PER_PAGE packet: {:?}",
                                                e
                                            );
                                            cursor += packet_len;
                                        }
                                    }
                                }
                                _ => {
                                    warn!(
                                        "Unknown packet ID: 0x{:04X} at cursor position {} of {}",
                                        packet_id,
                                        cursor,
                                        data.len()
                                    );

                                    // Try to determine packet size
                                    if let Some(size) = get_packet_size(packet_id) {
                                        // Known fixed-size packet
                                        if data.len() - cursor >= size {
                                            debug!(
                                                "Skipping known packet 0x{:04X} of size {}",
                                                packet_id, size
                                            );
                                            cursor += size;
                                        } else {
                                            debug!(
                                                "Incomplete packet 0x{:04X}, breaking",
                                                packet_id
                                            );
                                            break;
                                        }
                                    } else {
                                        // Unknown packet - check if it might be variable length
                                        if data.len() - cursor >= 4 {
                                            // Try to read as variable-length packet
                                            let potential_len = u16::from_le_bytes([
                                                data[cursor + 2],
                                                data[cursor + 3],
                                            ])
                                                as usize;
                                            if (4..=65535).contains(&potential_len)
                                                && data.len() - cursor >= potential_len
                                            {
                                                warn!(
                                                    "Skipping potential variable-length packet 0x{:04X} of size {}",
                                                    packet_id, potential_len
                                                );
                                                cursor += potential_len;
                                            } else {
                                                // Can't determine size, skip just the ID and hope for resync
                                                error!(
                                                    "Cannot determine size of packet 0x{:04X}, skipping 2 bytes",
                                                    packet_id
                                                );
                                                cursor += 2;
                                            }
                                        } else {
                                            // Not enough data to read length field
                                            debug!(
                                                "Not enough data to read packet length for 0x{:04X}",
                                                packet_id
                                            );
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // No data available right now
                        break;
                    }
                    Err(e) => {
                        return Err(NetworkError::from(e));
                    }
                }
            }
        }

        Ok(responses)
    }

    pub fn get_characters(&self) -> &[CharacterInfo] {
        &self.characters
    }

    pub fn update(&mut self) -> Result<Vec<CharServerResponse>, NetworkError> {
        // Send keepalive if needed
        self.send_keepalive()?;

        // Receive any pending packets
        self.receive_packets()
    }
}

pub fn char_client_update_system(
    mut client: Option<ResMut<CharServerClient>>,
    mut events: EventWriter<CharServerEvent>,
) {
    let Some(client) = client.as_deref_mut() else {
        return;
    };

    if !client.is_connected() {
        return;
    }

    match client.update() {
        Ok(responses) => {
            for response in responses {
                match response {
                    CharServerResponse::HcAcceptEnter(packet) => {
                        info!(
                            "Received character list with {} characters",
                            packet.characters.len()
                        );
                        events.write(CharServerEvent::CharacterListReceived(packet.characters));
                    }
                    CharServerResponse::HcNotifyZonesvr(packet) => {
                        info!("Received zone server info for map {}", packet.map_name);
                        events.write(CharServerEvent::ZoneServerInfo {
                            char_id: packet.char_id,
                            map_name: packet.map_name,
                            ip: packet.ip,
                            port: packet.port,
                        });
                    }
                    CharServerResponse::HcAcceptMakeChar(char_info) => {
                        info!("Character '{}' created successfully", char_info.name);
                        events.write(CharServerEvent::CharacterCreated(char_info));
                    }
                    CharServerResponse::HcAcceptDeleteChar => {
                        info!("Character deleted successfully");
                        events.write(CharServerEvent::CharacterDeleted);
                    }
                    CharServerResponse::HcRefuseMakeChar(error) => {
                        warn!("Character creation failed with error code: {}", error);
                        events.write(CharServerEvent::CharacterCreationFailed(error));
                    }
                    CharServerResponse::HcRefuseDeleteChar(error) => {
                        warn!("Character deletion failed with error code: {}", error);
                        events.write(CharServerEvent::CharacterDeletionFailed(error));
                    }
                    CharServerResponse::HcPing => {
                        debug!("Received ping response from character server");
                    }
                    CharServerResponse::HcCharacterList(packet) => {
                        info!(
                            "Character slot info: {} normal, {} premium, {} valid",
                            packet.normal_slots, packet.premium_slots, packet.valid_slots
                        );
                        events.write(CharServerEvent::CharacterSlotInfo {
                            normal_slots: packet.normal_slots,
                            premium_slots: packet.premium_slots,
                            valid_slots: packet.valid_slots,
                        });
                    }
                    CharServerResponse::HcBlockCharacter(packet) => {
                        let blocked_list: Vec<(u32, String)> = packet
                            .blocked_chars
                            .into_iter()
                            .map(|entry| (entry.char_id, entry.expire_date))
                            .collect();
                        info!(
                            "Received blocked character list with {} entries",
                            blocked_list.len()
                        );
                        events.write(CharServerEvent::BlockedCharacterList(blocked_list));
                    }
                    CharServerResponse::HcSecondPasswdLogin(packet) => {
                        info!(
                            "Pincode state: {} - {}",
                            packet.state,
                            packet.state_description()
                        );
                        events.write(CharServerEvent::PincodeState {
                            state: packet.state,
                            description: packet.state_description().to_string(),
                        });
                    }
                    CharServerResponse::HcAckCharinfoPerPage(packet) => {
                        info!(
                            "Received character list page with {} characters",
                            packet.characters.len()
                        );
                        events.write(CharServerEvent::CharacterListReceived(packet.characters));
                    }
                }
            }
        }
        Err(e) => {
            error!("Character client error: {:?}", e);
            events.write(CharServerEvent::ConnectionError(e));
        }
    }
}

#[derive(Event, Debug)]
pub enum CharServerEvent {
    CharacterListReceived(Vec<CharacterInfo>),
    ZoneServerInfo {
        char_id: u32,
        map_name: String,
        ip: [u8; 4],
        port: u16,
    },
    CharacterCreated(CharacterInfo),
    CharacterDeleted,
    CharacterCreationFailed(u8),
    CharacterDeletionFailed(u8),
    ConnectionError(NetworkError),
    CharacterSlotInfo {
        normal_slots: u8,
        premium_slots: u8,
        valid_slots: u8,
    },
    BlockedCharacterList(Vec<(u32, String)>), // (char_id, expire_date)
    PincodeState {
        state: u16,
        description: String,
    },
}
