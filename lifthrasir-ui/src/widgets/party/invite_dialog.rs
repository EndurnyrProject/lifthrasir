//! Incoming party-invite modal, shown through the shared `system_dialog`.
//!
//! An inbound [`PartyInviteNotified`] raises the reusable dialog with Accept
//! (primary) and Decline (secondary), tagged [`SystemDialogKind::PartyInvite`], and
//! records the invite in [`PendingPartyInvite`]. The dialog reports the press as a
//! [`SystemDialogChoice`] carrying that tag; the party handler claims it only when the
//! tag is `PartyInvite` *and* an invite is pending. A 30s TTL clears the pending invite
//! and closes the (single-open) dialog, matching the server's auto-decline.
//!
//! Correlation guarantee: the `kind` tag alone routes a choice to its raiser, so even if
//! a disconnect dialog and an invite are written in the same tick (only one dialog
//! actually spawns), a `Generic` disconnect choice can never be claimed as an invite
//! response. The single-open guard still prevents invites stacking on an open dialog.

use bevy::prelude::*;
use game_engine::presentation::ui::events::{
    DialogSeverity, ShowSystemDialog, SystemDialogChoice, SystemDialogKind,
};
use net_contract::commands::PartyInviteResponded;
use net_contract::events::PartyInviteNotified;

use crate::widgets::system_dialog::SystemDialogRoot;

/// Server auto-declines after this; the client mirrors it so a stale dialog never lingers.
const INVITE_TTL_SECS: f32 = 30.0;

/// The invite the on-screen dialog is deciding. `party_id == 0` means "none pending"
/// (mirroring `PartyState::in_party`); set only while the invite dialog is shown.
#[derive(Resource, Default)]
pub struct PendingPartyInvite {
    pub party_id: u32,
    pub party_name: String,
    pub inviter_name: String,
    timer: Timer,
}

impl PendingPartyInvite {
    pub fn is_pending(&self) -> bool {
        self.party_id != 0
    }

    fn set(&mut self, invite: &PartyInviteNotified) {
        self.party_id = invite.party_id;
        self.party_name = invite.party_name.clone();
        self.inviter_name = invite.inviter_name.clone();
        self.timer = Timer::from_seconds(INVITE_TTL_SECS, TimerMode::Once);
    }

    fn clear(&mut self) {
        *self = Self::default();
    }
}

/// Raise the shared dialog for the latest invite and record it as pending. Drops the
/// invite when a dialog is already open (mirroring the widget's single-open guard) so
/// invites never stack; the server's TTL re-offers if it still matters.
pub fn show_incoming_invite(
    mut invites: MessageReader<PartyInviteNotified>,
    existing: Query<(), With<SystemDialogRoot>>,
    mut pending: ResMut<PendingPartyInvite>,
    mut dialogs: MessageWriter<ShowSystemDialog>,
) {
    let Some(invite) = invites.read().last() else {
        return;
    };
    if !existing.is_empty() {
        return;
    }
    dialogs.write(ShowSystemDialog {
        severity: DialogSeverity::Info,
        kind: SystemDialogKind::PartyInvite,
        kicker: "Party".into(),
        title: "Party Invite".into(),
        message: format!(
            "{} invites you to {}.",
            invite.inviter_name, invite.party_name
        ),
        code: String::new(),
        button_label: "Accept".into(),
        secondary_label: "Decline".into(),
        confirm_state: None,
    });
    pending.set(invite);
}

/// Turn a dialog choice into a party response, but only when the choice came from an
/// invite dialog (`kind == PartyInvite`) and an invite is pending. A `Generic` choice
/// (e.g. the disconnect dialog's button) is ignored even if a race left it interleaved
/// with a pending invite, so it never becomes a spurious `PartyInviteResponded`.
pub fn claim_invite_choice(
    mut choices: MessageReader<SystemDialogChoice>,
    mut pending: ResMut<PendingPartyInvite>,
    mut responses: MessageWriter<PartyInviteResponded>,
) {
    let Some(choice) = choices
        .read()
        .filter(|choice| choice.kind == SystemDialogKind::PartyInvite)
        .last()
    else {
        return;
    };
    if !pending.is_pending() {
        return;
    }
    responses.write(PartyInviteResponded {
        party_id: pending.party_id,
        accept: choice.primary,
    });
    pending.clear();
}

/// After the TTL with no response, clear the pending invite and despawn the (single-open)
/// dialog. A late press then finds nothing pending and is a no-op.
pub fn expire_pending_invite(
    time: Res<Time>,
    mut pending: ResMut<PendingPartyInvite>,
    root: Query<Entity, With<SystemDialogRoot>>,
    mut commands: Commands,
) {
    if !pending.is_pending() {
        return;
    }
    if !pending.timer.tick(time.delta()).just_finished() {
        return;
    }
    pending.clear();
    if let Ok(root) = root.single() {
        commands.entity(root).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn invite() -> PartyInviteNotified {
        PartyInviteNotified {
            party_id: 42,
            party_name: "Wolfpack".into(),
            inviter_name: "Odin".into(),
        }
    }

    fn shown_dialogs(app: &App) -> Vec<ShowSystemDialog> {
        let messages = app.world().resource::<Messages<ShowSystemDialog>>();
        let mut cursor = messages.get_cursor();
        cursor.read(messages).cloned().collect()
    }

    fn responses(app: &App) -> Vec<PartyInviteResponded> {
        let messages = app.world().resource::<Messages<PartyInviteResponded>>();
        let mut cursor = messages.get_cursor();
        cursor.read(messages).cloned().collect()
    }

    #[test]
    fn incoming_invite_shows_dialog_and_sets_pending() {
        let mut app = App::new();
        app.add_message::<PartyInviteNotified>()
            .add_message::<ShowSystemDialog>()
            .init_resource::<PendingPartyInvite>()
            .add_systems(Update, show_incoming_invite);

        app.world_mut()
            .resource_mut::<Messages<PartyInviteNotified>>()
            .write(invite());
        app.update();

        let dialogs = shown_dialogs(&app);
        assert_eq!(dialogs.len(), 1, "one dialog raised");
        assert_eq!(dialogs[0].button_label, "Accept");
        assert_eq!(dialogs[0].secondary_label, "Decline");
        assert!(dialogs[0].confirm_state.is_none());

        let pending = app.world().resource::<PendingPartyInvite>();
        assert!(pending.is_pending());
        assert_eq!(pending.party_id, 42);
    }

    #[test]
    fn invite_dropped_when_a_dialog_is_already_open() {
        let mut app = App::new();
        app.add_message::<PartyInviteNotified>()
            .add_message::<ShowSystemDialog>()
            .init_resource::<PendingPartyInvite>()
            .add_systems(Update, show_incoming_invite);
        app.world_mut().spawn(SystemDialogRoot::default());

        app.world_mut()
            .resource_mut::<Messages<PartyInviteNotified>>()
            .write(invite());
        app.update();

        assert!(shown_dialogs(&app).is_empty(), "no dialog raised");
        assert!(
            !app.world().resource::<PendingPartyInvite>().is_pending(),
            "invite dropped, pending stays clear"
        );
    }

    #[test]
    fn choice_with_pending_writes_response_and_clears() {
        let mut app = App::new();
        app.add_message::<SystemDialogChoice>()
            .add_message::<PartyInviteResponded>()
            .init_resource::<PendingPartyInvite>()
            .add_systems(Update, claim_invite_choice);
        app.world_mut()
            .resource_mut::<PendingPartyInvite>()
            .set(&invite());

        app.world_mut()
            .resource_mut::<Messages<SystemDialogChoice>>()
            .write(SystemDialogChoice {
                primary: true,
                kind: SystemDialogKind::PartyInvite,
            });
        app.update();

        let written = responses(&app);
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].party_id, 42);
        assert!(written[0].accept, "accept mirrors primary");
        assert!(
            !app.world().resource::<PendingPartyInvite>().is_pending(),
            "response clears pending"
        );
    }

    #[test]
    fn choice_without_pending_writes_nothing() {
        let mut app = App::new();
        app.add_message::<SystemDialogChoice>()
            .add_message::<PartyInviteResponded>()
            .init_resource::<PendingPartyInvite>()
            .add_systems(Update, claim_invite_choice);

        app.world_mut()
            .resource_mut::<Messages<SystemDialogChoice>>()
            .write(SystemDialogChoice {
                primary: true,
                kind: SystemDialogKind::PartyInvite,
            });
        app.update();

        assert!(
            responses(&app).is_empty(),
            "a choice with no pending invite is ignored"
        );
    }

    #[test]
    fn generic_choice_is_ignored_even_with_pending() {
        let mut app = App::new();
        app.add_message::<SystemDialogChoice>()
            .add_message::<PartyInviteResponded>()
            .init_resource::<PendingPartyInvite>()
            .add_systems(Update, claim_invite_choice);
        app.world_mut()
            .resource_mut::<PendingPartyInvite>()
            .set(&invite());

        app.world_mut()
            .resource_mut::<Messages<SystemDialogChoice>>()
            .write(SystemDialogChoice {
                primary: true,
                kind: SystemDialogKind::Generic,
            });
        app.update();

        assert!(
            responses(&app).is_empty(),
            "a Generic choice (e.g. disconnect) never claims a pending invite"
        );
        assert!(
            app.world().resource::<PendingPartyInvite>().is_pending(),
            "pending invite survives an unrelated Generic choice"
        );
    }

    #[test]
    fn decline_choice_sends_non_accept_response() {
        let mut app = App::new();
        app.add_message::<SystemDialogChoice>()
            .add_message::<PartyInviteResponded>()
            .init_resource::<PendingPartyInvite>()
            .add_systems(Update, claim_invite_choice);
        app.world_mut()
            .resource_mut::<PendingPartyInvite>()
            .set(&invite());

        app.world_mut()
            .resource_mut::<Messages<SystemDialogChoice>>()
            .write(SystemDialogChoice {
                primary: false,
                kind: SystemDialogKind::PartyInvite,
            });
        app.update();

        let written = responses(&app);
        assert_eq!(written.len(), 1);
        assert!(!written[0].accept, "decline mirrors non-primary");
    }

    #[test]
    fn pending_invite_expires_after_ttl() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<PendingPartyInvite>()
            .add_systems(Update, expire_pending_invite);
        app.world_mut()
            .resource_mut::<PendingPartyInvite>()
            .set(&invite());

        app.update();
        assert!(
            app.world().resource::<PendingPartyInvite>().is_pending(),
            "still pending before the TTL"
        );

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(INVITE_TTL_SECS + 1.0));
        app.update();

        assert!(
            !app.world().resource::<PendingPartyInvite>().is_pending(),
            "TTL clears the pending invite"
        );
    }
}
