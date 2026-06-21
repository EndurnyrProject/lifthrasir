use game_engine::infrastructure::networking::{
    protocol::{
        zone::{
            CzEnter2Packet, CzNotifyActorinitPacket, ZoneClientPacket, ZoneProtocol,
            ZC_ACCEPT_ENTER, ZC_AID, ZC_REFUSE_ENTER,
        },
        ClientPacket, PacketSize, Protocol,
    },
    ZoneServerClient,
};

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
    use game_engine::infrastructure::networking::protocol::zone::ZoneContext;

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
    fn test_zone_protocol_name() {
        assert_eq!(ZoneProtocol::NAME, "Zone");
    }

    #[test]
    fn test_packet_id_consistency() {
        assert_eq!(CzEnter2Packet::PACKET_ID, 0x0363);
        assert_eq!(CzNotifyActorinitPacket::PACKET_ID, 0x007D);
    }
}

#[cfg(test)]
mod serialization_roundtrip_tests {
    use super::*;

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
