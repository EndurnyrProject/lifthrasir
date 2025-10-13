use super::errors::NetworkError;
use super::protocols::ro_zone::{
    CzEnter2Packet, CzNotifyActorinitPacket, ZcAcceptEnterPacket, ZcAidPacket, ZcRefuseEnterPacket,
    ZC_ACCEPT_ENTER, ZC_AID, ZC_REFUSE_ENTER,
};
use crate::domain::character::events::{
    ZoneAuthenticationFailed, ZoneAuthenticationSuccess, ZoneServerConnected,
    ZoneServerConnectionFailed,
};
use bevy::prelude::*;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

const CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_BUFFER_SIZE: usize = 1_048_576; // 1 MB - prevent DoS from unbounded buffer growth

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZoneConnectionState {
    Disconnected,
    Connecting,
    Authenticating,
    WaitingForMapLoad,
    Authenticated,
}

impl Default for ZoneConnectionState {
    fn default() -> Self {
        Self::Disconnected
    }
}

#[derive(Debug, Clone)]
pub struct ZoneSessionData {
    pub account_id: u32,
    pub character_id: u32,
    pub login_id1: u32,
    pub map_name: String,
    pub ip: String,
    pub port: u16,
    pub sex: u8,
}

#[derive(Resource)]
pub struct ZoneServerClient {
    connection: Option<TcpStream>,
    pub state: ZoneConnectionState,
    pub session_data: Option<ZoneSessionData>,
    pub spawn_position: Option<(u16, u16, u8)>, // (x, y, dir)
    pub server_tick: u32,
    buffer: Vec<u8>,
    connection_start: Option<Instant>,
}

impl Default for ZoneServerClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ZoneServerClient {
    pub fn new() -> Self {
        Self {
            connection: None,
            state: ZoneConnectionState::Disconnected,
            session_data: None,
            spawn_position: None,
            server_tick: 0,
            buffer: Vec::new(),
            connection_start: None,
        }
    }

    pub fn connect(&mut self, session_data: ZoneSessionData) -> Result<(), NetworkError> {
        info!(
            "Connecting to zone server at {}:{}",
            session_data.ip, session_data.port
        );

        let addr = format!("{}:{}", session_data.ip, session_data.port);
        let socket_addr = addr
            .parse::<std::net::SocketAddr>()
            .map_err(|_| NetworkError::InvalidPacket)?;

        let stream = TcpStream::connect_timeout(&socket_addr, CONNECTION_TIMEOUT)?;
        stream.set_nonblocking(true)?;

        self.connection = Some(stream);
        self.state = ZoneConnectionState::Connecting;
        self.session_data = Some(session_data);
        self.connection_start = Some(Instant::now());

        // Send CZ_ENTER2 packet immediately after connection
        self.send_cz_enter()?;

        info!("Sent CZ_ENTER2 to zone server");
        self.state = ZoneConnectionState::Authenticating;

        Ok(())
    }

    pub fn disconnect(&mut self) {
        if let Some(stream) = self.connection.take() {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
        self.state = ZoneConnectionState::Disconnected;
        self.session_data = None;
        self.spawn_position = None;
        self.server_tick = 0;
        self.buffer.clear();
        self.connection_start = None;
    }

    pub fn is_connected(&self) -> bool {
        self.connection.is_some()
    }

    pub fn send_cz_enter(&mut self) -> Result<(), NetworkError> {
        let session = self
            .session_data
            .as_ref()
            .ok_or(NetworkError::InvalidPacket)?;

        let packet = CzEnter2Packet {
            account_id: session.account_id,
            char_id: session.character_id,
            auth_code: session.login_id1,
            client_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as u32,
            unknown: 0,
            sex: session.sex,
        };

        self.send_packet(&packet.serialize())?;
        info!("Sent CZ_ENTER2 packet to zone server");
        Ok(())
    }

    pub fn send_actor_init(&mut self) -> Result<(), NetworkError> {
        let packet = CzNotifyActorinitPacket::build();
        self.send_packet(&packet)?;
        info!("Sent CZ_NOTIFY_ACTORINIT to zone server");
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

    fn receive_packets(&mut self) -> Result<Vec<ZoneServerResponse>, NetworkError> {
        let mut responses = Vec::new();

        if let Some(ref mut stream) = self.connection {
            let mut temp_buffer = [0u8; 4096];

            loop {
                match stream.read(&mut temp_buffer) {
                    Ok(0) => {
                        // Connection closed
                        self.disconnect();
                        return Err(NetworkError::UnexpectedDisconnect);
                    }
                    Ok(n) => {
                        debug!("Received {} bytes from zone server", n);
                        self.buffer.extend_from_slice(&temp_buffer[..n]);

                        // Check buffer size to prevent DoS
                        if self.buffer.len() > MAX_BUFFER_SIZE {
                            error!(
                                "Receive buffer exceeded max size ({} bytes). Disconnecting.",
                                MAX_BUFFER_SIZE
                            );
                            self.disconnect();
                            return Err(NetworkError::UnexpectedDisconnect);
                        }

                        // Process all complete packets in buffer
                        while self.buffer.len() >= 2 {
                            let packet_id = u16::from_le_bytes([self.buffer[0], self.buffer[1]]);
                            debug!("Processing packet ID: 0x{:04X}", packet_id);

                            match packet_id {
                                ZC_ACCEPT_ENTER => {
                                    const PACKET_SIZE: usize = 13;
                                    if self.buffer.len() < PACKET_SIZE {
                                        break; // Wait for more data
                                    }

                                    match ZcAcceptEnterPacket::parse(&self.buffer[..PACKET_SIZE]) {
                                        Ok(packet) => {
                                            info!(
                                                "Zone authentication successful! Spawn at ({}, {}) dir {}",
                                                packet.x, packet.y, packet.dir
                                            );
                                            self.spawn_position =
                                                Some((packet.x, packet.y, packet.dir));
                                            self.server_tick = packet.start_time;
                                            self.state = ZoneConnectionState::WaitingForMapLoad;
                                            responses
                                                .push(ZoneServerResponse::ZcAcceptEnter(packet));
                                            self.buffer.drain(..PACKET_SIZE);
                                        }
                                        Err(e) => {
                                            error!("Failed to parse ZC_ACCEPT_ENTER: {:?}", e);
                                            self.buffer.drain(..PACKET_SIZE);
                                        }
                                    }
                                }
                                ZC_AID => {
                                    const PACKET_SIZE: usize = 6;
                                    if self.buffer.len() < PACKET_SIZE {
                                        break; // Wait for more data
                                    }

                                    match ZcAidPacket::parse(&self.buffer[..PACKET_SIZE]) {
                                        Ok(packet) => {
                                            info!(
                                                "Received account ID from zone server: {}",
                                                packet.account_id
                                            );
                                            // This packet is informational only, no need to emit event
                                            self.buffer.drain(..PACKET_SIZE);
                                        }
                                        Err(e) => {
                                            error!("Failed to parse ZC_AID: {:?}", e);
                                            self.buffer.drain(..PACKET_SIZE);
                                        }
                                    }
                                }
                                ZC_REFUSE_ENTER => {
                                    const PACKET_SIZE: usize = 3;
                                    if self.buffer.len() < PACKET_SIZE {
                                        break; // Wait for more data
                                    }

                                    match ZcRefuseEnterPacket::parse(&self.buffer[..PACKET_SIZE]) {
                                        Ok(packet) => {
                                            warn!(
                                                "Zone authentication failed: {} (code: {})",
                                                packet.error_description(),
                                                packet.error_code
                                            );
                                            responses
                                                .push(ZoneServerResponse::ZcRefuseEnter(packet));
                                            self.buffer.drain(..PACKET_SIZE);
                                        }
                                        Err(e) => {
                                            error!("Failed to parse ZC_REFUSE_ENTER: {:?}", e);
                                            self.buffer.drain(..PACKET_SIZE);
                                        }
                                    }
                                }
                                _ => {
                                    error!("Unknown zone packet ID: 0x{:04X}. Disconnecting to prevent stream corruption.", packet_id);
                                    self.disconnect();
                                    return Err(NetworkError::InvalidPacket);
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

    pub fn update(&mut self) -> Result<Vec<ZoneServerResponse>, NetworkError> {
        // Check for connection timeout
        if let Some(start_time) = self.connection_start {
            if start_time.elapsed() > CONNECTION_TIMEOUT
                && self.state == ZoneConnectionState::Authenticating
            {
                error!("Zone server connection timeout");
                self.disconnect();
                return Err(NetworkError::Timeout);
            }
        }

        // Receive any pending packets
        self.receive_packets()
    }
}

#[derive(Debug)]
pub enum ZoneServerResponse {
    ZcAcceptEnter(ZcAcceptEnterPacket),
    ZcRefuseEnter(ZcRefuseEnterPacket),
}

/// System to handle zone server connection requests
pub fn zone_connection_system(
    mut zone_info_events: EventReader<
        crate::domain::character::events::ZoneServerInfoReceivedEvent,
    >,
    mut zone_client: Option<ResMut<ZoneServerClient>>,
    mut char_client: Option<ResMut<super::CharServerClient>>,
    mut connected_events: EventWriter<ZoneServerConnected>,
    mut failed_events: EventWriter<ZoneServerConnectionFailed>,
    commands: Commands,
) {
    for event in zone_info_events.read() {
        info!(
            "Received zone server info for map '{}' at {}:{}",
            event.map_name, event.server_ip, event.server_port
        );

        // Disconnect from character server
        if let Some(ref mut client) = char_client.as_deref_mut() {
            info!("Disconnecting from character server");
            client.disconnect();
        }

        // Create session data from event
        let session_data = ZoneSessionData {
            account_id: event.account_id,
            character_id: event.char_id,
            login_id1: event.login_id1,
            map_name: event.map_name.clone(),
            ip: event.server_ip.clone(),
            port: event.server_port,
            sex: event.sex,
        };

        // Connect to zone server (resource should always exist, initialized in plugin)
        if let Some(ref mut client) = zone_client.as_deref_mut() {
            match client.connect(session_data) {
                Ok(()) => {
                    info!("Successfully connected to zone server");
                    connected_events.write(ZoneServerConnected);
                }
                Err(e) => {
                    error!("Failed to connect to zone server: {:?}", e);
                    failed_events.write(ZoneServerConnectionFailed {
                        reason: format!("{:?}", e),
                    });
                }
            }
        } else {
            error!("ZoneServerClient resource not available - this should not happen!");
            failed_events.write(ZoneServerConnectionFailed {
                reason: "ZoneServerClient resource not initialized".to_string(),
            });
        }
    }
}

/// System to handle zone server packet responses
pub fn zone_packet_handler_system(
    mut zone_client: Option<ResMut<ZoneServerClient>>,
    mut auth_success_events: EventWriter<ZoneAuthenticationSuccess>,
    mut auth_failed_events: EventWriter<ZoneAuthenticationFailed>,
) {
    let Some(client) = zone_client.as_deref_mut() else {
        return;
    };

    if !client.is_connected() {
        return;
    }

    match client.update() {
        Ok(responses) => {
            for response in responses {
                match response {
                    ZoneServerResponse::ZcAcceptEnter(packet) => {
                        info!(
                            "Zone authentication successful! Spawn: ({}, {}) dir: {}",
                            packet.x, packet.y, packet.dir
                        );
                        auth_success_events.write(ZoneAuthenticationSuccess {
                            spawn_x: packet.x,
                            spawn_y: packet.y,
                            spawn_dir: packet.dir,
                            server_tick: packet.start_time,
                        });
                    }
                    ZoneServerResponse::ZcRefuseEnter(packet) => {
                        warn!(
                            "Zone authentication failed: {} (code {})",
                            packet.error_description(),
                            packet.error_code
                        );
                        auth_failed_events.write(ZoneAuthenticationFailed {
                            error_code: packet.error_code,
                        });
                        client.disconnect();
                    }
                }
            }
        }
        Err(e) => {
            error!("Zone client error: {:?}", e);
            auth_failed_events.write(ZoneAuthenticationFailed { error_code: 255 });
            client.disconnect();
        }
    }
}
