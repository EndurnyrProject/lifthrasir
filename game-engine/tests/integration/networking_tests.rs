use game_engine::infrastructure::networking::{
    protocol::{
        character::{
            CharacterClientPacket, CharacterInfo, CharacterProtocol, ChEnterPacket,
            ChSelectCharPacket, HC_ACCEPT_ENTER, HC_NOTIFY_ZONESVR,
        },
        login::{
            CaLoginPacket, LoginClientPacket, LoginProtocol, AC_ACCEPT_LOGIN, AC_REFUSE_LOGIN,
        },
        zone::{
            CzEnter2Packet, CzNotifyActorinitPacket, ZoneClientPacket, ZoneProtocol,
            ZC_ACCEPT_ENTER, ZC_AID, ZC_REFUSE_ENTER,
        },
        ClientPacket, PacketSize, Protocol,
    },
    CharServerClient, LoginClient, ZoneServerClient,
};

#[cfg(test)]
mod login_protocol_tests {
    use super::*;

    #[test]
    fn test_login_packet_sizes() {
        // AC_ACCEPT_LOGIN is variable length
        assert!(matches!(
            LoginProtocol::packet_size(AC_ACCEPT_LOGIN),
            PacketSize::Variable { .. }
        ));

        // AC_REFUSE_LOGIN is fixed 23 bytes
        assert_eq!(
            LoginProtocol::packet_size(AC_REFUSE_LOGIN),
            PacketSize::Fixed(23)
        );
    }

    #[test]
    fn test_ca_login_serialization() {
        let packet = CaLoginPacket::new("testuser", "testpass", 55);
        let bytes = packet.serialize();

        // Verify packet structure
        assert_eq!(bytes.len(), 56); // Fixed size
        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), 0x0064); // CA_LOGIN
    }

    #[test]
    fn test_login_client_packet_enum() {
        let packet = LoginClientPacket::CaLogin(CaLoginPacket::new("test", "pass", 55));
        assert_eq!(packet.packet_id(), 0x0064); // CA_LOGIN

        let bytes = packet.serialize();
        assert_eq!(bytes.len(), 56);
    }
}

#[cfg(test)]
mod character_protocol_tests {
    use super::*;

    #[test]
    fn test_character_packet_sizes() {
        // HC_ACCEPT_ENTER is variable length
        assert!(matches!(
            CharacterProtocol::packet_size(HC_ACCEPT_ENTER),
            PacketSize::Variable { .. }
        ));

        // HC_NOTIFY_ZONESVR is fixed 28 bytes
        assert_eq!(
            CharacterProtocol::packet_size(HC_NOTIFY_ZONESVR),
            PacketSize::Fixed(28)
        );
    }

    #[test]
    fn test_ch_enter_serialization() {
        let packet = ChEnterPacket::new(12345, 67890, 11111);
        let bytes = packet.serialize();

        // Verify packet structure
        assert_eq!(bytes.len(), 15); // Fixed size
        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), 0x0065); // CH_ENTER

        // Verify account ID
        assert_eq!(
            u32::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]),
            12345
        );
    }

    #[test]
    fn test_ch_select_char_serialization() {
        let packet = ChSelectCharPacket::new(2);
        let bytes = packet.serialize();

        assert_eq!(bytes.len(), 3); // Fixed size
        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), 0x0066); // CH_SELECT_CHAR
        assert_eq!(bytes[2], 2); // Slot
    }

    #[test]
    fn test_character_client_packet_enum() {
        let packet = CharacterClientPacket::ChEnter(ChEnterPacket::new(12345, 67890, 11111));
        assert_eq!(packet.packet_id(), 0x0065); // CH_ENTER

        let bytes = packet.serialize();
        assert_eq!(bytes.len(), 15);
    }
}

#[cfg(test)]
mod zone_protocol_tests {
    use super::*;

    #[test]
    fn test_zone_packet_sizes() {
        // ZC_ACCEPT_ENTER is fixed 13 bytes
        assert_eq!(
            ZoneProtocol::packet_size(ZC_ACCEPT_ENTER),
            PacketSize::Fixed(13)
        );

        // ZC_AID is fixed 6 bytes
        assert_eq!(ZoneProtocol::packet_size(ZC_AID), PacketSize::Fixed(6));

        // ZC_REFUSE_ENTER is fixed 3 bytes
        assert_eq!(
            ZoneProtocol::packet_size(ZC_REFUSE_ENTER),
            PacketSize::Fixed(3)
        );
    }

    #[test]
    fn test_cz_enter2_serialization() {
        let auth_code = [0u8; 17];
        let packet = CzEnter2Packet::new(12345, 67890, auth_code);
        let bytes = packet.serialize();

        // Verify packet structure
        assert_eq!(bytes.len(), 26); // Fixed size
        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), 0x0363); // CZ_ENTER2
    }

    #[test]
    fn test_cz_notify_actorinit_serialization() {
        let packet = CzNotifyActorinitPacket::new();
        let bytes = packet.serialize();

        assert_eq!(bytes.len(), 2); // Just packet ID
        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), 0x007D); // CZ_NOTIFY_ACTORINIT
    }

    #[test]
    fn test_zone_client_packet_enum() {
        let auth_code = [0u8; 17];
        let packet = ZoneClientPacket::CzEnter2(CzEnter2Packet::new(12345, 67890, auth_code));
        assert_eq!(packet.packet_id(), 0x0363); // CZ_ENTER2

        let bytes = packet.serialize();
        assert_eq!(bytes.len(), 26);
    }
}

#[cfg(test)]
mod client_wrapper_tests {
    use super::*;
    use game_engine::infrastructure::networking::protocol::{
        character::CharacterContext, login::LoginContext, zone::ZoneContext,
    };

    #[test]
    fn test_login_client_creation() {
        let client = LoginClient::new();
        assert!(!client.is_connected());
        assert_eq!(client.attempt_count(), 0);
        assert_eq!(client.last_error(), None);
    }

    #[test]
    fn test_login_client_default() {
        let client = LoginClient::default();
        assert!(!client.is_connected());
    }

    #[test]
    fn test_char_server_client_creation() {
        let client = CharServerClient::with_session(12345, 67890, 11111);
        assert!(!client.is_connected());
        assert_eq!(client.characters().len(), 0);
        assert!(client.zone_server_info().is_none());
    }

    #[test]
    fn test_char_server_client_with_context() {
        let context = CharacterContext::with_session(12345, 67890, 11111);
        let client = CharServerClient::new(context);
        assert!(!client.is_connected());
    }

    #[test]
    fn test_zone_server_client_creation() {
        let client = ZoneServerClient::with_session(12345, 67890);
        assert!(!client.is_connected());
        assert!(!client.is_ready());
        assert!(!client.entered_world());
        assert!(!client.received_aid());
    }

    #[test]
    fn test_zone_server_client_with_context() {
        let context = ZoneContext::with_session(12345, 67890);
        let client = ZoneServerClient::new(context);
        assert!(!client.is_connected());
    }
}

#[cfg(test)]
mod protocol_consistency_tests {
    use super::*;

    #[test]
    fn test_login_protocol_name() {
        assert_eq!(LoginProtocol::NAME, "Login");
    }

    #[test]
    fn test_character_protocol_name() {
        assert_eq!(CharacterProtocol::NAME, "Character");
    }

    #[test]
    fn test_zone_protocol_name() {
        assert_eq!(ZoneProtocol::NAME, "Zone");
    }

    #[test]
    fn test_packet_id_consistency() {
        // Login packets
        assert_eq!(CaLoginPacket::PACKET_ID, 0x0064);

        // Character packets
        assert_eq!(ChEnterPacket::PACKET_ID, 0x0065);
        assert_eq!(ChSelectCharPacket::PACKET_ID, 0x0066);

        // Zone packets
        assert_eq!(CzEnter2Packet::PACKET_ID, 0x0363);
        assert_eq!(CzNotifyActorinitPacket::PACKET_ID, 0x007D);
    }
}

#[cfg(test)]
mod serialization_roundtrip_tests {
    use super::*;

    #[test]
    fn test_login_packet_roundtrip() {
        let original = CaLoginPacket::new("user123", "pass456", 55);
        let bytes = original.serialize();

        // Verify we can extract the packet ID
        let packet_id = u16::from_le_bytes([bytes[0], bytes[1]]);
        assert_eq!(packet_id, CaLoginPacket::PACKET_ID);

        // Verify length
        assert_eq!(bytes.len(), 56);
    }

    #[test]
    fn test_character_packet_roundtrip() {
        let original = ChEnterPacket::new(12345, 67890, 11111);
        let bytes = original.serialize();

        let packet_id = u16::from_le_bytes([bytes[0], bytes[1]]);
        assert_eq!(packet_id, ChEnterPacket::PACKET_ID);
        assert_eq!(bytes.len(), 15);
    }

    #[test]
    fn test_zone_packet_roundtrip() {
        let auth_code = [0u8; 17];
        let original = CzEnter2Packet::new(12345, 67890, auth_code);
        let bytes = original.serialize();

        let packet_id = u16::from_le_bytes([bytes[0], bytes[1]]);
        assert_eq!(packet_id, CzEnter2Packet::PACKET_ID);
        assert_eq!(bytes.len(), 26);
    }
}
