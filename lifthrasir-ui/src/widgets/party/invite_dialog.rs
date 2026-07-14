//! Incoming party-invite modal, shown through the shared `system_dialog`.
//!
//! An inbound [`PartyInviteNotified`] raises the reusable dialog with Accept
//! (primary) and Decline (secondary), tagged [`SystemDialogKind::PartyInvite`], and
//! records the invite in [`PendingPartyInvite`]. The dialog reports the press as a
//! [`SystemDialogChoice`] carrying its kind and correlation; the party handler claims
//! only the exact pending invite. A 30s TTL clears the pending invite and closes only
//! its matching dialog, matching the server's auto-decline.
//!
//! A monotonically increasing local token prevents an expired choice from claiming a
//! later party invite. The single-open guard still prevents invites stacking.

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
    correlation: Option<u64>,
    next_correlation: u64,
}

impl PendingPartyInvite {
    pub fn is_pending(&self) -> bool {
        self.party_id != 0
    }

    fn set(&mut self, invite: &PartyInviteNotified) {
        self.next_correlation = self.next_correlation.wrapping_add(1).max(1);
        self.party_id = invite.party_id;
        self.party_name = invite.party_name.clone();
        self.inviter_name = invite.inviter_name.clone();
        self.timer = Timer::from_seconds(INVITE_TTL_SECS, TimerMode::Once);
        self.correlation = Some(self.next_correlation);
    }

    pub(crate) fn clear(&mut self) {
        self.party_id = 0;
        self.party_name.clear();
        self.inviter_name.clear();
        self.timer = Timer::default();
        self.correlation = None;
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
    pending.set(invite);
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
        correlation: pending.correlation,
    });
}

/// Turn a dialog choice into a party response only when its kind and token match the
/// pending invite. Unrelated and stale choices remain no-ops.
pub fn claim_invite_choice(
    mut choices: MessageReader<SystemDialogChoice>,
    mut pending: ResMut<PendingPartyInvite>,
    mut responses: MessageWriter<PartyInviteResponded>,
) {
    if !pending.is_pending() {
        return;
    }
    let Some(choice) = choices
        .read()
        .filter(|choice| {
            choice.kind == SystemDialogKind::PartyInvite
                && choice.correlation == pending.correlation
        })
        .last()
    else {
        return;
    };
    responses.write(PartyInviteResponded {
        party_id: pending.party_id,
        accept: choice.primary,
    });
    pending.clear();
}

/// After the TTL, clear the pending invite and despawn only its matching dialog.
pub fn expire_pending_invite(
    time: Res<Time>,
    mut pending: ResMut<PendingPartyInvite>,
    roots: Query<(Entity, &SystemDialogRoot)>,
    mut commands: Commands,
) {
    if !pending.is_pending() {
        return;
    }
    if !pending.timer.tick(time.delta()).just_finished() {
        return;
    }
    let correlation = pending.correlation;
    pending.clear();
    if let Some((entity, _)) = roots
        .iter()
        .find(|(_, root)| root.matches(SystemDialogKind::PartyInvite, correlation))
    {
        commands.entity(entity).despawn();
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
        assert!(dialogs[0].correlation.is_some());

        let pending = app.world().resource::<PendingPartyInvite>();
        assert!(pending.is_pending());
        assert_eq!(pending.party_id, 42);
        assert_eq!(dialogs[0].correlation, pending.correlation);
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
        let correlation = app.world().resource::<PendingPartyInvite>().correlation;

        app.world_mut()
            .resource_mut::<Messages<SystemDialogChoice>>()
            .write(SystemDialogChoice {
                primary: true,
                kind: SystemDialogKind::PartyInvite,
                correlation,
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
                correlation: None,
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
                correlation: None,
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
    fn wrong_correlation_choice_writes_nothing_and_keeps_pending() {
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
                correlation: Some(999),
            });
        app.update();

        assert!(responses(&app).is_empty());
        assert!(app.world().resource::<PendingPartyInvite>().is_pending());
    }

    #[test]
    fn stale_choice_cannot_claim_a_later_invite() {
        let mut app = App::new();
        app.add_message::<SystemDialogChoice>()
            .add_message::<PartyInviteResponded>()
            .init_resource::<PendingPartyInvite>()
            .add_systems(Update, claim_invite_choice);
        let mut pending = app.world_mut().resource_mut::<PendingPartyInvite>();
        pending.set(&invite());
        let stale = pending.correlation;
        pending.clear();
        pending.set(&invite());
        let current = pending.correlation;
        drop(pending);
        assert_ne!(stale, current);

        app.world_mut()
            .resource_mut::<Messages<SystemDialogChoice>>()
            .write(SystemDialogChoice {
                primary: true,
                kind: SystemDialogKind::PartyInvite,
                correlation: stale,
            });
        app.update();

        assert!(responses(&app).is_empty());
        assert!(app.world().resource::<PendingPartyInvite>().is_pending());
        assert_eq!(
            app.world().resource::<PendingPartyInvite>().correlation,
            current
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
        let correlation = app.world().resource::<PendingPartyInvite>().correlation;

        app.world_mut()
            .resource_mut::<Messages<SystemDialogChoice>>()
            .write(SystemDialogChoice {
                primary: false,
                kind: SystemDialogKind::PartyInvite,
                correlation,
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

    #[test]
    fn invite_timeout_does_not_close_an_unrelated_dialog() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<PendingPartyInvite>()
            .add_systems(Update, expire_pending_invite);
        app.world_mut()
            .resource_mut::<PendingPartyInvite>()
            .set(&invite());
        let unrelated = app.world_mut().spawn(SystemDialogRoot::default()).id();

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(INVITE_TTL_SECS + 1.0));
        app.update();

        assert!(app.world().get_entity(unrelated).is_ok());
        assert!(!app.world().resource::<PendingPartyInvite>().is_pending());
    }

    #[test]
    fn invite_timeout_closes_its_matching_dialog() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<PendingPartyInvite>()
            .add_systems(Update, expire_pending_invite);
        app.world_mut()
            .resource_mut::<PendingPartyInvite>()
            .set(&invite());
        let correlation = app.world().resource::<PendingPartyInvite>().correlation;
        let owned = app
            .world_mut()
            .spawn(SystemDialogRoot::new(
                None,
                SystemDialogKind::PartyInvite,
                correlation,
            ))
            .id();

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(INVITE_TTL_SECS + 1.0));
        app.update();

        assert!(app.world().get_entity(owned).is_err());
        assert!(!app.world().resource::<PendingPartyInvite>().is_pending());
    }
}
