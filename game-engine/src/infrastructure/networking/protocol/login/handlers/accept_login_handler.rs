use crate::infrastructure::networking::{
    errors::NetworkError,
    protocol::{
        login::{
            protocol::{LoginContext, LoginProtocol},
            server_packets::AcAcceptLoginPacket,
            types::ServerInfo,
        },
        traits::{EventWriter, PacketHandler},
    },
};
use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::auto_add_event;

/// Event emitted when login is accepted
#[derive(Message, Debug, Clone)]
#[auto_add_event(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct LoginAccepted {
    pub account_id: u32,
    pub login_id1: u32,
    pub login_id2: u32,
    pub last_login_ip: u32,
    pub sex: u8,
    pub server_list: Vec<ServerInfo>,
    pub username: String,
}

/// Handler for AC_ACCEPT_LOGIN packet
///
/// This handler processes successful login responses and emits a
/// LoginAccepted event that can be consumed by Bevy systems.
pub struct AcceptLoginHandler;

impl PacketHandler<LoginProtocol> for AcceptLoginHandler {
    type Packet = AcAcceptLoginPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        context: &mut LoginContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        info!(
            "Login accepted! Account ID: {}, {} servers available",
            packet.account_id,
            packet.server_list.len()
        );

        // Log server list for debugging
        for (idx, server) in packet.server_list.iter().enumerate() {
            debug!(
                "  [{}] {} - {}:{} ({} users, type: {:?})",
                idx,
                server.name,
                server.ip_string(),
                server.port,
                server.users,
                server.server_type
            );
        }

        let username = context.username.clone().unwrap_or_default();

        context.reset();

        let event = LoginAccepted {
            account_id: packet.account_id,
            login_id1: packet.login_id1,
            login_id2: packet.login_id2,
            last_login_ip: packet.last_login_ip,
            sex: packet.sex,
            server_list: packet.server_list,
            username,
        };

        event_writer.send_event(Box::new(event));

        Ok(())
    }
}
