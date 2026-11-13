use super::inventory::inventory_item;
use crate::proto::aesir::net;
use net_contract::dto::ChatChannel;
use net_contract::events::{ChatHeard, ChatNotice, EmoteShown, EntityNamed};

pub fn name_response(n: net::NameResponse) -> EntityNamed {
    EntityNamed {
        gid: n.gid,
        name: n.name,
        party_name: n.party_name,
        guild_name: n.guild_name,
        position_name: n.position_name,
    }
}

pub fn chat_line(c: net::ChatLine) -> ChatHeard {
    let channel = match c.channel() {
        net::ChatChannel::ChatLocal => ChatChannel::Local,
        net::ChatChannel::ChatParty => ChatChannel::Party,
        net::ChatChannel::ChatGuild => ChatChannel::Guild,
        net::ChatChannel::ChatTrade => ChatChannel::Trade,
        net::ChatChannel::ChatWhisper => ChatChannel::Whisper,
    };
    ChatHeard {
        channel,
        sender_gid: c.sender_gid,
        sender_name: c.sender_name,
        text: if c.raw_text.is_empty() {
            c.text
        } else {
            net_contract::dto::display_skill_text(&c.raw_text)
        },
        whisper_to: c.whisper_to,
        links: c.links.into_iter().map(inventory_item).collect(),
    }
}

pub fn chat_notice(n: net::ChatNotice) -> ChatNotice {
    ChatNotice {
        error: n.kind() == net::chat_notice::Kind::Error,
        message: n.text,
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
    fn chat_line_maps_speaker_channel_and_links() {
        let heard = chat_line(net::ChatLine {
            channel: net::ChatChannel::ChatWhisper.into(),
            sender_gid: 150001,
            sender_name: "Freyja".into(),
            text: "take my \u{fffc}".into(),
            whisper_to: "Ygor".into(),
            links: vec![net::InventoryItem {
                nameid: 610,
                ..Default::default()
            }],
            ..Default::default()
        });

        assert_eq!(heard.channel, ChatChannel::Whisper);
        assert_eq!(heard.sender_gid, 150001);
        assert_eq!(heard.sender_name, "Freyja");
        assert_eq!(heard.text, "take my \u{fffc}");
        assert_eq!(heard.whisper_to, "Ygor");
        assert_eq!(heard.links[0].nameid, 610);
    }

    #[test]
    fn talkie_chat_uses_raw_bytes_and_sanitizes_them() {
        let heard = chat_line(net::ChatLine {
            sender_gid: 1,
            text: "fallback".into(),
            raw_text: vec![b'A', 0xff, b'\n', b'B'],
            ..Default::default()
        });
        assert_eq!(heard.text, "A\u{fffd}B");
    }

    #[test]
    fn chat_notice_maps_kind_to_the_error_flag() {
        let notice = |kind: net::chat_notice::Kind| {
            chat_notice(net::ChatNotice {
                kind: kind.into(),
                text: "Unknown command".into(),
            })
        };
        assert!(notice(net::chat_notice::Kind::Error).error);
        assert!(!notice(net::chat_notice::Kind::Info).error);
        assert_eq!(
            notice(net::chat_notice::Kind::Info).message,
            "Unknown command"
        );
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
