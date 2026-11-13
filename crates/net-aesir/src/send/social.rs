use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{QuinnetClient, client_connected};
use net_contract::commands::{ChatSent, EmoteSent, GmCommandSent};
use net_contract::dto::ChatChannel;

use crate::channels::GAMEPLAY;
use crate::envelope::Body;
use crate::proto::aesir::net::{self, ChatSend, EmoteRequest, GmCommand};
use crate::zone::{QuicZoneState, ZonePhase};

fn chat_body(c: &ChatSent) -> Body {
    let channel = match c.channel {
        ChatChannel::Local => net::ChatChannel::ChatLocal,
        ChatChannel::Party => net::ChatChannel::ChatParty,
        ChatChannel::Guild => net::ChatChannel::ChatGuild,
        ChatChannel::Trade => net::ChatChannel::ChatTrade,
        ChatChannel::Whisper => net::ChatChannel::ChatWhisper,
    };
    Body::ChatSend(ChatSend {
        channel: channel.into(),
        text: c.text.clone(),
        whisper_to: c.whisper_to.clone(),
        link_indices: c.link_indices.clone(),
    })
}

fn gm_body(c: &GmCommandSent) -> Body {
    Body::GmCommand(GmCommand {
        command: c.command.clone(),
    })
}

fn emote_body(c: &EmoteSent) -> Body {
    Body::EmoteRequest(EmoteRequest {
        r#type: c.emote_type,
    })
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_chat_requests(
    mut events: MessageReader<ChatSent>,
    mut commands: MessageReader<GmCommandSent>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        commands.clear();
        return;
    }
    let bodies = events
        .read()
        .map(chat_body)
        .chain(commands.read().map(gm_body));
    for body in bodies {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, body) {
            error!("failed to send chat: {e}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_emote_requests(
    mut events: MessageReader<EmoteSent>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let Err(e) = zone.send(&mut client, GAMEPLAY, emote_body(ev)) {
            error!("failed to send EmoteRequest: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_body_carries_channel_text_recipient_and_links() {
        let body = chat_body(&ChatSent {
            channel: ChatChannel::Whisper,
            text: "hello".into(),
            whisper_to: "Freyja".into(),
            link_indices: vec![7],
        });
        let Body::ChatSend(sent) = body else {
            panic!("expected Body::ChatSend, got {body:?}");
        };
        assert_eq!(sent.channel(), net::ChatChannel::ChatWhisper);
        assert_eq!(sent.text, "hello");
        assert_eq!(sent.whisper_to, "Freyja");
        assert_eq!(sent.link_indices, [7]);
    }

    #[test]
    fn gm_body_carries_the_command_line() {
        let body = gm_body(&GmCommandSent {
            command: "item 501 10".into(),
        });
        match body {
            Body::GmCommand(GmCommand { command }) => assert_eq!(command, "item 501 10"),
            other => panic!("expected Body::GmCommand, got {other:?}"),
        }
    }

    #[test]
    fn emote_body_carries_type() {
        let body = emote_body(&EmoteSent { emote_type: 4 });
        match body {
            Body::EmoteRequest(EmoteRequest { r#type }) => assert_eq!(r#type, 4),
            other => panic!("expected Body::EmoteRequest, got {other:?}"),
        }
    }
}
