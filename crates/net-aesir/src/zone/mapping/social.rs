use crate::proto::aesir::net;
use net_contract::events::{ChatHeard, EmoteShown, EntityNamed};

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
        message: if c.raw_message.is_empty() {
            c.message
        } else {
            net_contract::dto::display_skill_text(&c.raw_message)
        },
    }
}

pub fn emotion(e: net::Emotion) -> EmoteShown {
    EmoteShown {
        gid: e.gid,
        emote_type: e.r#type,
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
            raw_message: vec![],
        });

        assert_eq!(heard.gid, 150001);
        assert_eq!(heard.message, "hello world");
    }

    #[test]
    fn talkie_chat_uses_raw_bytes_and_sanitizes_them() {
        let heard = chat_message(net::ChatMessage {
            gid: 1,
            message: "fallback".into(),
            raw_message: vec![b'A', 0xff, b'\n', b'B'],
        });
        assert_eq!(heard.message, "A\u{fffd}B");
    }

    #[test]
    fn emotion_maps_gid_and_type() {
        let shown = emotion(net::Emotion {
            gid: 150001,
            r#type: 4,
        });

        assert_eq!(shown.gid, 150001);
        assert_eq!(shown.emote_type, 4);
    }
}
