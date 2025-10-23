use crate::infrastructure::networking::{
    errors::NetworkError,
    protocol::{
        login::{
            protocol::{LoginContext, LoginProtocol},
            server_packets::AcRefuseLoginPacket,
        },
        traits::{EventWriter, PacketHandler},
    },
};
use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::auto_add_event;

/// Event emitted when login is refused
#[derive(Message, Debug, Clone)]
#[auto_add_event(plugin = crate::app::authentication_plugin::AuthenticationPlugin)]
pub struct LoginRefused {
    pub username: String,
    pub error_code: u8,
    pub error_message: String,
    pub block_date: Option<String>,
}

/// Handler for AC_REFUSE_LOGIN packet
///
/// This handler processes login rejection responses and emits a
/// LoginRefused event with error details.
pub struct RefuseLoginHandler;

impl PacketHandler<LoginProtocol> for RefuseLoginHandler {
    type Packet = AcRefuseLoginPacket;

    fn handle(
        &self,
        packet: Self::Packet,
        context: &mut LoginContext,
        event_writer: &mut dyn EventWriter,
    ) -> Result<(), NetworkError> {
        let error_message = packet.error_message();
        let block_date = packet.block_date_string();

        error!(
            "Login refused: {} (code {})",
            error_message, packet.error_code
        );

        if let Some(ref date) = block_date {
            error!("Account blocked until: {}", date);
        }

        context.record_error(packet.error_code);

        let username = context.username.clone().unwrap_or_default();

        let event = LoginRefused {
            username,
            error_code: packet.error_code,
            error_message: error_message.to_string(),
            block_date,
        };

        event_writer.send_event(Box::new(event));

        Ok(())
    }
}
