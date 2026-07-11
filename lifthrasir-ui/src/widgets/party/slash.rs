//! Party slash-command parsing + dispatch.
//!
//! `chat_input_control` (`chat_box.rs`) calls [`parse_party_slash`] before sending a
//! normal chat message; a recognized command is queued as [`PartySlashSubmitted`]
//! instead, and [`dispatch_party_slash`] turns it into the matching outbound command.
//! Keeping the mapping here (rather than inline in `chat_box.rs`) keeps chat_box
//! ignorant of party semantics beyond "is this a party slash".

use bevy::prelude::*;
use net_contract::commands::{PartyCreateRequested, PartyInviteRequested, PartyInviteResponded};

use super::PendingPartyInvite;

/// A recognized party slash command, parsed from raw chat input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartySlash {
    Create(String),
    Invite(String),
    Accept,
    Decline,
}

/// Queued by `chat_input_control` on a recognized slash; [`dispatch_party_slash`]
/// consumes it into the matching outbound command.
#[derive(Message, Debug, Clone)]
pub struct PartySlashSubmitted(pub PartySlash);

/// Parse one chat line into a party slash command. Returns `None` for anything else
/// (normal chat, an unrecognized slash, or `/pcreate`/`/pinvite` missing their required
/// name), so the caller falls through to sending it as a normal chat message.
pub fn parse_party_slash(input: &str) -> Option<PartySlash> {
    let (command, rest) = input.trim().split_once(' ').unwrap_or((input.trim(), ""));
    let arg = rest.trim().to_string();
    match command {
        "/pcreate" => (!arg.is_empty()).then_some(PartySlash::Create(arg)),
        "/pinvite" => (!arg.is_empty()).then_some(PartySlash::Invite(arg)),
        "/paccept" => Some(PartySlash::Accept),
        "/pdecline" => Some(PartySlash::Decline),
        _ => None,
    }
}

/// Turn each queued [`PartySlashSubmitted`] into the matching outbound command.
/// `Accept`/`Decline` need a pending invite recorded in [`PendingPartyInvite`] (set from
/// `PartyInviteNotified`); with none pending they are silent no-ops, mirroring
/// `claim_invite_choice`.
pub fn dispatch_party_slash(
    mut submitted: MessageReader<PartySlashSubmitted>,
    mut pending: ResMut<PendingPartyInvite>,
    mut create: MessageWriter<PartyCreateRequested>,
    mut invite: MessageWriter<PartyInviteRequested>,
    mut respond: MessageWriter<PartyInviteResponded>,
) {
    for PartySlashSubmitted(slash) in submitted.read() {
        match slash {
            PartySlash::Create(name) => {
                create.write(PartyCreateRequested { name: name.clone() });
            }
            PartySlash::Invite(name) => {
                invite.write(PartyInviteRequested {
                    target_char_id: 0,
                    target_name: name.clone(),
                });
            }
            PartySlash::Accept if pending.is_pending() => {
                respond.write(PartyInviteResponded {
                    party_id: pending.party_id,
                    accept: true,
                });
                pending.clear();
            }
            PartySlash::Decline if pending.is_pending() => {
                respond.write(PartyInviteResponded {
                    party_id: pending.party_id,
                    accept: false,
                });
                pending.clear();
            }
            PartySlash::Accept | PartySlash::Decline => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_create_with_trimmed_name() {
        assert_eq!(
            parse_party_slash("/pcreate   Wolfpack  "),
            Some(PartySlash::Create("Wolfpack".to_string()))
        );
    }

    #[test]
    fn parses_invite_with_trimmed_name() {
        assert_eq!(
            parse_party_slash("/pinvite  Odin  "),
            Some(PartySlash::Invite("Odin".to_string()))
        );
    }

    #[test]
    fn parses_accept_and_decline() {
        assert_eq!(parse_party_slash("/paccept"), Some(PartySlash::Accept));
        assert_eq!(parse_party_slash("/pdecline"), Some(PartySlash::Decline));
    }

    #[test]
    fn create_and_invite_without_a_name_are_none() {
        assert_eq!(parse_party_slash("/pcreate"), None);
        assert_eq!(parse_party_slash("/pcreate   "), None);
        assert_eq!(parse_party_slash("/pinvite"), None);
    }

    #[test]
    fn normal_chat_and_unknown_slash_are_none() {
        assert_eq!(parse_party_slash("hello world"), None);
        assert_eq!(parse_party_slash("/foo bar"), None);
    }

    fn responses(app: &App) -> Vec<PartyInviteResponded> {
        let messages = app.world().resource::<Messages<PartyInviteResponded>>();
        let mut cursor = messages.get_cursor();
        cursor.read(messages).cloned().collect()
    }

    fn creates(app: &App) -> Vec<PartyCreateRequested> {
        let messages = app.world().resource::<Messages<PartyCreateRequested>>();
        let mut cursor = messages.get_cursor();
        cursor.read(messages).cloned().collect()
    }

    fn invites(app: &App) -> Vec<PartyInviteRequested> {
        let messages = app.world().resource::<Messages<PartyInviteRequested>>();
        let mut cursor = messages.get_cursor();
        cursor.read(messages).cloned().collect()
    }

    fn dispatch_app() -> App {
        let mut app = App::new();
        app.add_message::<PartySlashSubmitted>()
            .add_message::<PartyCreateRequested>()
            .add_message::<PartyInviteRequested>()
            .add_message::<PartyInviteResponded>()
            .init_resource::<PendingPartyInvite>()
            .add_systems(Update, dispatch_party_slash);
        app
    }

    fn submit(app: &mut App, slash: PartySlash) {
        app.world_mut()
            .resource_mut::<Messages<PartySlashSubmitted>>()
            .write(PartySlashSubmitted(slash));
    }

    #[test]
    fn create_dispatches_party_create_requested() {
        let mut app = dispatch_app();
        submit(&mut app, PartySlash::Create("Wolfpack".to_string()));
        app.update();

        let written = creates(&app);
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].name, "Wolfpack");
    }

    #[test]
    fn invite_dispatches_party_invite_requested_by_name() {
        let mut app = dispatch_app();
        submit(&mut app, PartySlash::Invite("Odin".to_string()));
        app.update();

        let written = invites(&app);
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].target_char_id, 0);
        assert_eq!(written[0].target_name, "Odin");
    }

    #[test]
    fn accept_with_pending_invite_writes_response_and_clears_pending() {
        let mut app = dispatch_app();
        app.world_mut()
            .resource_mut::<PendingPartyInvite>()
            .party_id = 42;
        submit(&mut app, PartySlash::Accept);
        app.update();

        let written = responses(&app);
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].party_id, 42);
        assert!(written[0].accept);
        assert!(!app.world().resource::<PendingPartyInvite>().is_pending());
    }

    #[test]
    fn decline_with_pending_invite_writes_non_accept_response() {
        let mut app = dispatch_app();
        app.world_mut()
            .resource_mut::<PendingPartyInvite>()
            .party_id = 42;
        submit(&mut app, PartySlash::Decline);
        app.update();

        let written = responses(&app);
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].party_id, 42);
        assert!(!written[0].accept);
    }

    #[test]
    fn accept_without_pending_invite_is_a_no_op() {
        let mut app = dispatch_app();
        submit(&mut app, PartySlash::Accept);
        app.update();

        assert!(responses(&app).is_empty(), "no pending invite, no response");
    }

    #[test]
    fn decline_without_pending_invite_is_a_no_op() {
        let mut app = dispatch_app();
        submit(&mut app, PartySlash::Decline);
        app.update();

        assert!(responses(&app).is_empty(), "no pending invite, no response");
    }
}
