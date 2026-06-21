use crate::infrastructure::networking::quic::proto::aesir::net;
use crate::infrastructure::networking::zone_messages::{ChatHeard, EntityNamed};

pub fn name_response(n: net::NameResponse) -> EntityNamed {
    EntityNamed {
        gid: n.gid,
        name: n.name,
        party_name: n.party_name,
        guild_name: n.guild_name,
        position_name: n.position_name,
    }
}

pub fn chat_message(c: net::ChatMessage) -> ChatHeard {
    ChatHeard {
        gid: c.gid,
        message: c.message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_response_maps_all_labels() {
        let named = name_response(net::NameResponse {
            gid: 150001,
            name: "Alice".into(),
            party_name: "Party".into(),
            guild_name: "Guild".into(),
            position_name: "Leader".into(),
        });

        assert_eq!(named.gid, 150001);
        assert_eq!(named.name, "Alice");
        assert_eq!(named.party_name, "Party");
        assert_eq!(named.guild_name, "Guild");
        assert_eq!(named.position_name, "Leader");
    }

    #[test]
    fn chat_message_maps_gid_and_text() {
        let heard = chat_message(net::ChatMessage {
            gid: 150001,
            message: "hello world".into(),
        });

        assert_eq!(heard.gid, 150001);
        assert_eq!(heard.message, "hello world");
    }
}
